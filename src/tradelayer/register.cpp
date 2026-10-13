#include <tradelayer/register.h>
#include <tradelayer/ce.h>
#include <tradelayer/log.h>
#include <tradelayer/tradelayer.h>
#include <tradelayer/uint256_extensions.h>
#include <tradelayer/externfns.h>

#include <stdint.h>
#include <map>

using namespace mastercore;

CCriticalSection cs_register;

// list of all amounts for all addresses for all contracts, map is unsorted
std::unordered_map<std::string, Register> mastercore::mp_register_map;

/**
 * Creates an empty register.
 */
Register::Register()
{
    my_it = mp_record.begin();
}

/**
 * Resets the internal iterator.
 *
 * @return Identifier of the first register element.
 */
uint32_t Register::init()
{
    uint32_t contractId = 0;
    my_it = mp_record.begin();
    if (my_it != mp_record.end()) {
        contractId = my_it->first;
    }

    return contractId;
}

/**
 * Advances the internal iterator.
 *
 * @return Identifier of the register element before the update.
 */
uint32_t Register::next()
{
    uint32_t ret = 0;
    if (my_it != mp_record.end()) {
        ret = my_it->first;
        ++my_it;
    }
    return ret;
}

bool Register::isBegin()
{
    if (my_it == mp_record.begin()) {
        return true;
    } else {
        return false;
    }
}

int64_t Register::getPosMarkPrice(const uint32_t contractId, bool isOracle) const
{
    if(isOracle)
    {
        int64_t oracleTwap = mastercore::getOracleTwap(contractId, 1);
        int64_t oracleLag = mastercore::getOracleTwap(contractId, 3);

        if(oracleLag*0.965>=oracleTwap){
            oracleTwap = oracleLag*0.965;
        }
            
        return oracleTwap;
    }

    // refine this: native contracts mark price
    return 0;
}  

/**
 * Shared liquidation/bankruptcy price computation (adversarial re-review fix
 * for finding #1): used by BOTH Register::getLiquidationPrice (RPC display via
 * getFullContractRecord) and Register::setBankruptcyPrice (the consensus
 * writer of the BANKRUPTCY_PRICE record via updateAllEntry ->
 * set_bankruptcy_price_onmap), so the two paths can no longer drift apart.
 *
 *   dliqPrice = 1 / ( totalMargin / (position * notionalSize) + COIN / entryPrice )
 *
 * totalMargin is the full margin backing the position (position *
 * margin_requirement for getLiquidationPrice, the reserved init margin for
 * setBankruptcyPrice). The entry-price term is COIN / entryPrice: the old
 * setBankruptcyPrice used 1 / entryPrice there (plain integer division ==
 * 0), which was yet another divergence between the two formulas.
 *
 * Every division below is a floating-point division on purpose: the old
 * setBankruptcyPrice divided int64_t / int64_t first (truncating to 0 for any
 * realistic ratio, and raising SIGFPE when position * notionalSize was 0),
 * then computed 1 / (0 + 0) == inf and fed it to DoubleToInt64() ->
 * StrToInt64("inf"), which throws and aborts block connection.
 *
 * The result is scaled by COIN *before* truncating to int64_t (scaling after
 * the cast drops the fraction and yields 0 for any price below 1.0) and
 * bounded so the multiplication and the cast cannot overflow. NaN and
 * infinity fail both comparisons below, so those cases fall through to the 0
 * return as well.
 *
 * Returns 0 when no valid positive price can be computed (zero position, zero
 * notional size, non-positive entry price, non-finite/non-positive result),
 * matching the historical "price = 0" fallback.
 */
static int64_t computeLiquidationPrice(double totalMargin, int64_t position, uint32_t notionalSize, int64_t entryPrice)
{
    // zero position or zero notional size would make the margin term divide
    // by zero; a non-positive entry price makes the reciprocal term
    // meaningless (insertEntry() only ever records positive prices)
    if (position == 0 || notionalSize == 0 || entryPrice <= 0) {
        return 0;
    }

    const double firstFactor = totalMargin / ((double) position * (double) notionalSize);
    const double secondFactor = (double) COIN / (double) entryPrice;
    const double denominator = firstFactor + secondFactor;

    const double dliqPrice = (denominator != 0) ? 1 / denominator : 0;

    if (0 < dliqPrice && dliqPrice < (double) (MAX_INT_8_BYTES / COIN))
    {
        return (int64_t) (dliqPrice * COIN);
    }

    PrintToLog("%s(): liqPrice is not a valid positive price: %lf, liquidation price = 0 !\n",__func__, dliqPrice);
    return 0;
}

int64_t Register::getLiquidationPrice(const uint32_t contractId, const uint32_t notionalSize, const uint64_t marginRequirement) const
{
    const int64_t position = getRecord(contractId, CONTRACT_POSITION);
    const int64_t entryPrice = getPosEntryPrice(contractId);

    // full margin backing the position; computed in double so the
    // multiplication cannot overflow the integer range
    const double marginTotal = (double) position * (double) marginRequirement;

    const int64_t liqPrice = computeLiquidationPrice(marginTotal, position, notionalSize, entryPrice);

    PrintToLog("%s(): liqPrice (int64_t) : %d\n",__func__, liqPrice);

    return liqPrice;

}

int64_t Register::getUPNL(const uint32_t contractId, const uint32_t notionalSize, bool isOracle, bool quoted)
{

    const int64_t position = getRecord(contractId, CONTRACT_POSITION);
    const int64_t entryPrice = getPosEntryPrice(contractId);
    const int64_t markPrice  = getPosMarkPrice(contractId, isOracle);

    if(msc_debug_liquidation_enginee)
    {
         PrintToLog("%s(): inside getUPNL, position: %d. entryPrice: %d, exitPrice: %d, notionalSize: %d\n",__func__, position, entryPrice, markPrice, notionalSize);
    }

    const arith_uint256 factor = ConvertTo256((abs(position) * notionalSize) / COIN );
    const arith_uint256 dEntryPrice = ConvertTo256(entryPrice / COIN);
    
    const arith_uint256 dMarkPrice = ConvertTo256(markPrice / COIN);
    int64_t diff = markPrice - entryPrice;
    if(diff==markPrice){diff=0;}

    const arith_uint256 dDiff = ConvertTo256(abs(diff));

    PrintToLog("%s(): dEntryPrice: %d, dExitPrice: %d, factor: %d, diff: %d\n",__func__, ConvertTo64(dEntryPrice), ConvertTo64(dMarkPrice), ConvertTo64(factor), diff);

    arith_uint256 aUPNL = 0;

    if(quoted)
    {

        aUPNL = (entryPrice != 0 && markPrice != 0) ?  (dDiff * (factor / dEntryPrice)) / dMarkPrice : 0;

    } else {
        aUPNL  = (factor * dDiff);
    }

    // re-converting to int64_t
    const uint64_t uiUPNL = ConvertTo64(aUPNL);
    int64_t iUPNL = 0;

    if ((diff > 0 && position < 0) || (diff < 0 && position > 0)) {
        iUPNL = (int64_t) -uiUPNL;
    } else {
        iUPNL = (int64_t) uiUPNL;
    }

    // updating UPNL Register
    const int64_t oldUPNL = getRecord(contractId, UPNL);
    PrintToLog("%s():oldUPNL: %d, iUPNL: %d\n",__func__, oldUPNL, iUPNL);

    if(oldUPNL != iUPNL) {
        updateRecord(contractId, iUPNL, UPNL);
        PrintToLog("%s():updating record, contractId: %d\n",__func__, contractId);

    }

    if(msc_debug_liquidation_enginee)
    {
       PrintToLog("%s(): entryPrice(int64_t): %d, exitPrice(int64_t): %d, notionalsize: %d\n",__func__, entryPrice, markPrice, notionalSize);
       PrintToLog("%s():UPNL(int): %d, quoted?: %d\n",__func__, iUPNL, quoted);
    }


    return iUPNL;
}

bool Register::setBankruptcyPrice(const uint32_t contractId, const uint32_t notionalSize, int64_t initMargin, bool isOracle, bool quoted)
{
    const int64_t position = getRecord(contractId, CONTRACT_POSITION);
    const int64_t entryPrice = getPosEntryPrice(contractId);

    // This is the consensus writer of the BANKRUPTCY_PRICE record
    // (updateAllEntry -> set_bankruptcy_price_onmap -> here), so it must use
    // the exact same fixed-point computation as the RPC display path
    // (getLiquidationPrice): the shared computeLiquidationPrice() helper
    // (adversarial re-review fix for finding #1).
    //
    // The old body computed
    //   1 / (initMargin / (position * notionalSize) + 1 / entryPrice)
    // with plain integer divisions: initMargin / (position * notionalSize)
    // truncated to 0 for any realistic ratio and raised SIGFPE when
    // position * notionalSize was 0, 1 / entryPrice truncated to 0, and
    // 1 / (0 + 0) == inf then went through DoubleToInt64() ->
    // StrToInt64("inf"), which throws (uncaught during block connection).
    // The result was also stored through a "const int", truncating the int64_t
    // to int32_t. initMargin is the full reserved margin for the position.
    const int64_t bankruptcyPrice = computeLiquidationPrice((double) initMargin, position, notionalSize, entryPrice);

    PrintToLog("%s(): bankruptcyPrice: %d\n",__func__, bankruptcyPrice);

    return updateRecord(contractId, bankruptcyPrice, BANKRUPTCY_PRICE);
}


/**
 * Updates amount for the given record type.
 *
 * Negative balances are only permitted for contracts amount, PNL and UPNL.
 *
 */
bool Register::updateRecord(uint32_t contractId, int64_t amount, RecordType ttype)
{
    bool fPNL = (ttype == PNL)? true : false;

    if (RECORD_TYPE_COUNT <= ttype || (amount == 0  && !fPNL) ) {
        PrintToLog("%s(): ERROR: ttype (%d) is wrong, or updating amount is zero (%d)\n", __func__, ttype, amount);
        return false;
    }

    bool fUpdated = false;
    int64_t now64 = mp_record[contractId].balance[ttype];

    PrintToLog("%s(): now64: %d\n", __func__, now64);

    if (isOverflow(now64, amount) && fPNL) {
        PrintToLog("%s(): ERROR: arithmetic overflow [%d + %d]\n", __func__, now64, amount);
        return false;
    }

    if ((CONTRACT_POSITION != ttype) && (UPNL != ttype) && (PNL != ttype) && fPNL && ((now64 + amount) < 0)) {
        // NOTE:
        PrintToLog("%s(): ERROR: Negative balances are only permitted for contracts amount, or UPNL\n",__func__);
        return false;
    } else {

        now64 += amount;
        mp_record[contractId].balance[ttype] = (fPNL) ? amount : now64;
        fUpdated = true;
    }

    return fUpdated;
}


/**
 *  Inserting price and amount
 *
 */
bool Register::insertEntry(uint32_t contractId, int64_t amount, int64_t price)
{
    bool bRet = false;

    // extra protection
    if(amount == 0 || price == 0) {
        return bRet;
    }

    RecordMap::iterator it = mp_record.find(contractId);
    std::pair<int64_t,int64_t> p (amount, price);

    if (it != mp_record.end()) {
        PositionRecord& record = it->second;
        Entries& entries = record.entries;
        entries.push_back(p);
        bRet = true;
    } else {
         PositionRecord posRec;
         Entries& entries = posRec.entries;
         entries.push_back(p);
         mp_record.insert(std::make_pair(contractId, posRec));
         bRet = true;
    }


    return bRet;

}

// Entry price for liquidations
int64_t Register::getEntryPrice(uint32_t contractId, int64_t amount) const
{
    RecordMap::const_iterator it = mp_record.find(contractId);

    if (it != mp_record.end())
    {
        const PositionRecord& record = it->second;
        const Entries& entries = record.entries;

        arith_uint256 total = 0;

        // setting remaining
        int64_t remaining = amount;

        auto itt = entries.begin();

        while(remaining > 0 && itt != entries.end()) {
          // process to calculate it
          const std::pair<int64_t,int64_t>& p = *itt;

          const int64_t& ramount = p.first;
          const int64_t& rprice = p.second;

          int64_t part = 0;

          if(remaining - ramount >= 0) {
              part = ramount;
          } else {
              part = remaining;
          }

          total += ConvertTo256(part * rprice);
          PrintToLog("%s(): part: %d, total: %d\n",__func__, part, ConvertTo64(total));
          remaining -= part;
          PrintToLog("%s(): remaining after loop: %d\n",__func__, remaining);
          PrintToLog("-------------------------------------\n\n");
          ++itt;
        }

        // using 256bits for big calculations
        const arith_uint256 aAmount = ConvertTo256(amount);
        const arith_uint256 aNewPrice = (amount != 0) ? DivideAndRoundUp(total, aAmount) : arith_uint256(0);
        const int64_t newPrice = ConvertTo64(aNewPrice);

        PrintToLog("%s(): total sum: %d, amount: %d, newPrice: %d\n",__func__, ConvertTo64(total), amount, newPrice);

        return newPrice;
    }


    return false;
}

// Entry price for full position
int64_t Register::getPosEntryPrice(uint32_t contractId, std::string& address) const
{
    //this is getting the avg. entry price of all open interest in a contract isn't it?
    //we should add the address string
    int64_t price = 0;
    RecordMap::const_iterator it = mp_record.find(contractId);
    //insert use of the address as part of the if logic for the loop
    if (it != mp_record.end())
    {
        const PositionRecord& record = it->second;
        const Entries& entries = record.entries;
        arith_uint256 total = 0;  // contracts * price
        int64_t amount = 0; // sum of contracts

        for(const auto& i : entries)
        {
            const int64_t& ramount = i.first;
            const int64_t& rprice = i.second;

            PrintToLog("%s(): ramount: %d, rprice: %d\n",__func__, ramount, rprice);

            total += ConvertTo256(ramount * rprice);
            amount += ramount;
        }

        const arith_uint256 aAmount = ConvertTo64(amount);
        if(amount==0){return 0;}
        const arith_uint256 aPrice = (amount != 0) ? DivideAndRoundUp(total, aAmount) : arith_uint256(0);
        price = ConvertTo64(aPrice);

    }

    return price;
}

int64_t Register::getPosEntryPrice(uint32_t contractId) const
{
    //this is getting the avg. entry price of all open interest in a contract isn't it?
    //we should add the address string
    int64_t price = 0;
    RecordMap::const_iterator it = mp_record.find(contractId);
    //insert use of the address as part of the if logic for the loop
    if (it != mp_record.end())
    {
        const PositionRecord& record = it->second;
        const Entries& entries = record.entries;
        arith_uint256 total = 0;  // contracts * price
        int64_t amount = 0; // sum of contracts

        for(const auto& i : entries)
        {
            const int64_t& ramount = i.first;
            const int64_t& rprice = i.second;

            PrintToLog("%s(): ramount: %d, rprice: %d\n",__func__, ramount, rprice);

            total += ConvertTo256(ramount * rprice);
            amount += ramount;
        }

        const arith_uint256 aAmount = ConvertTo64(amount);
        if(amount==0){return 0;}
        const arith_uint256 aPrice = (amount != 0) ? DivideAndRoundUp(total, aAmount) : arith_uint256(0);
        price = ConvertTo64(aPrice);

    }

    return price;
}


/**
 * Returns any record.
 *
 */
int64_t Register::getRecord(uint32_t contractId, RecordType ttype) const
{

    int64_t amount = 0;

    if (RECORD_TYPE_COUNT <= ttype) {
        return amount;
    }


    RecordMap::const_iterator it = mp_record.find(contractId);

    if (it != mp_record.end()) {
        const PositionRecord& record = it->second;
        amount = record.balance[ttype];
    }

    return amount;
}


const Entries* Register::getEntries(const uint32_t contractId) const
{
    const auto it = mp_record.find(contractId);

    if (it != mp_record.end()) {
        const PositionRecord& record = it->second;
        return &(record.entries);
    }

    return static_cast<Entries*>(nullptr);
}


/**
 * Compares the Register with another Register and returns true, if they are equal.
 *
 * @param rhs  The other Register
 * @return True, if both Registers are equal
 */
bool Register::operator==(const Register& rhs) const
{
    if (mp_record.size() != rhs.mp_record.size()) {
        return false;
    }
    RecordMap::const_iterator pc1 = mp_record.begin();
    RecordMap::const_iterator pc2 = rhs.mp_record.begin();

    for (unsigned int i = 0; i < mp_record.size(); ++i) {
        if (pc1->first != pc2->first) {
            return false;
        }
        const PositionRecord& record1 = pc1->second;
        const PositionRecord& record2 = pc2->second;

        for (int ttype = 0; ttype < RECORD_TYPE_COUNT; ++ttype) {
            if (record1.balance[ttype] != record2.balance[ttype]) {
                return false;
            }
        }
        ++pc1;
        ++pc2;
    }

    if((pc1 != mp_record.end()) || (pc2 != rhs.mp_record.end())) {
         return false;
    }


    return true;
}


// look at Record for an address, for a given contractId
int64_t mastercore::getContractRecord(const std::string& address, uint32_t contractId, RecordType ttype)
{
    int64_t balance = 0;
    if (RECORD_TYPE_COUNT <= ttype) {
        return 0;
    }

    LOCK(cs_register);
    std::unordered_map<std::string, Register>::const_iterator my_it = mp_register_map.find(address);
    if (my_it != mp_register_map.end()) {
        balance = (my_it->second).getRecord(contractId, ttype);
    }

    return balance;
}

bool mastercore::getFullContractRecord(const std::string& address, uint32_t contractId, UniValue& position_obj, const CDInfo::Entry& cd)
{
    LOCK(cs_register);
    std::unordered_map<std::string, Register>::iterator my_it = mp_register_map.find(address);
    if (my_it != mp_register_map.end()) {
        Register& reg = my_it->second;
        //entry price
        const int64_t entryPrice = reg.getPosEntryPrice(contractId);
        if(entryPrice==0){return false;}
        position_obj.pushKV("entry_price", FormatDivisibleMP(entryPrice));
        // position
        const int64_t position = reg.getRecord(contractId, CONTRACT_POSITION);
        position_obj.pushKV("position", FormatIndivisibleMP(position));
        // liquidation price
        const int64_t liqPrice = reg.getLiquidationPrice(contractId, cd.notional_size, cd.margin_requirement);
        position_obj.pushKV("BANKRUPTCY_PRICE", FormatDivisibleMP(liqPrice));
        // position margin
        const int64_t posMargin = reg.getRecord(contractId, MARGIN);
        position_obj.pushKV("position_margin", FormatDivisibleMP(posMargin));
        // upnl
        const int64_t upnl = reg.getUPNL(contractId, cd.notional_size, cd.isOracle(), cd.isInverseQuoted());
        position_obj.pushKV("upnl", FormatDivisibleMP(upnl, true));

        return true;
    }

    return false;
}

// return true if everything is ok
bool mastercore::update_register_map(const std::string& who, uint32_t contractId, int64_t amount, RecordType ttype)
{
    if (0 == amount && ttype != PNL) {
        PrintToLog("%s(%s, %u=0x%X, %+d, ttype=%d) ERROR: amount is zero\n", __func__, who, contractId, contractId, amount, ttype);
        return false;
    }
    if (ttype >= RECORD_TYPE_COUNT) {
        PrintToLog("%s(%s, %u=0x%X, %+d, ttype=%d) ERROR: invalid record type\n", __func__, who, contractId, contractId, amount, ttype);
        return false;
    }

    bool bRet = false;
    int64_t before = 0;
    int64_t after = 0;

    LOCK(cs_register);

    before = mastercore::getContractRecord(who, contractId, ttype);

    std::unordered_map<std::string, Register>::iterator my_it = mp_register_map.find(who);
    if (my_it == mp_register_map.end()) {
        // insert an empty element
        my_it = (mp_register_map.insert(std::make_pair(who, Register()))).first;
    }

    Register& reg = my_it->second;
    bRet = reg.updateRecord(contractId, amount, ttype);

    after = getContractRecord(who, contractId, ttype);

    if (!bRet) {
        if(before != after){
            PrintToLog("%s(): ERROR: Positions should be the same (%s), before (%d), after(%d)\n", __func__, who, before, after);
        }

        PrintToLog("%s(): ERROR: no position updated for (%s), before (%d), after(%d)\n", __func__, who, before, after);
    }

    return bRet;
}

// return true if everything is ok
bool mastercore::set_bankruptcy_price_onmap(const std::string& who, const uint32_t& contractId, const uint32_t& notionalSize, const int64_t& initMargin)
{

    LOCK(cs_register);
    std::unordered_map<std::string, Register>::iterator my_it = mp_register_map.find(who);
    if (my_it == mp_register_map.end()) {
        // insert an empty element
        my_it = (mp_register_map.insert(std::make_pair(who, Register()))).first;
    }

    Register& reg = my_it->second;
    return reg.setBankruptcyPrice(contractId, notionalSize, initMargin);

}

bool mastercore::settlement_pnl(uint32_t contractId, uint32_t notional_size, bool isOracle, bool isInverseQuoted, uint32_t collateral_currency)
{
    bool bRet = false;

    LOCK(cs_register);

    for(auto my_it = mp_register_map.begin(); my_it != mp_register_map.end(); ++my_it)
    {
        const std::string& who = my_it->first;
        Register& reg = my_it->second;
        const int64_t upnl = reg.getUPNL(contractId, notional_size, isOracle, isInverseQuoted);
        const int64_t oldPNL = reg.getRecord(contractId, PNL);
        const int64_t newUPNL = upnl - oldPNL;

        PrintToLog("%s(): upnl: %d, oldPNL: %d, newUPNL: %d\n",__func__, upnl, oldPNL, newUPNL);

        if (0 != newUPNL)
        {
            PrintToLog("%s(): updating register map (because newUPNL is not zero)\n",__func__);

            update_register_map(who, contractId, newUPNL, MARGIN);
            update_register_map(who, contractId, newUPNL + oldPNL, PNL);
            update_tally_map(who, collateral_currency, newUPNL, BALANCE);
            bRet = true;
        }
    }

    return bRet;
}

// return true if everything is ok
bool mastercore::reset_leverage_register(const std::string& who, uint32_t contractId)
{
    bool bRet = false;

    LOCK(cs_register);

    const int64_t rleverage = mastercore::getContractRecord(who, contractId, LEVERAGE);

    std::unordered_map<std::string, Register>::iterator my_it = mp_register_map.find(who);
    if (my_it == mp_register_map.end()) {
        PrintToLog("%s(): address (%s) not found for this contract(%d)\n",__func__, who, contractId);
        return bRet;
    }

    Register& reg = my_it->second;

    // cleaning
    bRet = reg.updateRecord(contractId, -rleverage, LEVERAGE);

    // // default leverage : 1
    // bRet2 = reg.updateRecord(contractId, 1, LEVERAGE);

    return bRet;
}

bool mastercore::insert_entry(const std::string& who, uint32_t contractId, int64_t amount, int64_t price)
{
    bool bRet = false;
    if (0 == amount) {
        PrintToLog("%s(%s, %u=0x%X, %+d) ERROR: amount of contracts is zero\n", __func__, who, contractId, contractId, amount);
        return bRet;
    }

    if (0 >= price) {
        PrintToLog("%s(%s, %u=0x%X, %+d) ERROR: price of contracts is zero or less\n", __func__, who, contractId, contractId, amount);
        return bRet;
    }

    LOCK(cs_register);

    std::unordered_map<std::string, Register>::iterator my_it = mp_register_map.find(who);
    if (my_it == mp_register_map.end()) {
        // insert an empty element
        my_it = (mp_register_map.insert(std::make_pair(who, Register()))).first;
    }

    Register& reg = my_it->second;

    bRet = reg.insertEntry(contractId, amount, price);

    // entry price of full position
    //reg.getPosEntryPrice(contractId, who);

    //PrintToLog("%s(): entryPrice(full position): %d, contractId: %d, address: %s\n",__func__, entryPrice, contractId, who);

    return bRet;
}



bool mastercore::decrease_entry(const std::string& who, uint32_t contractId, int64_t amount, int64_t price, bool inverse, int64_t collateral_currency, int direction)
{
    bool bRet = false;
    if (0 == amount) {
        PrintToLog("%s(%s, %u=0x%X, %+d) ERROR: amount of contracts is zero\n", __func__, who, contractId, contractId, amount);
        return bRet;
    }

    LOCK(cs_register);

    std::unordered_map<std::string, Register>::iterator my_it = mp_register_map.find(who);
    if (my_it == mp_register_map.end()) {
        // insert an empty element
        my_it = (mp_register_map.insert(std::make_pair(who, Register()))).first;
    } //look out for possible memory err risk, this seems to insulate against an overflow but what about buffer and seg-fault risk?

    Register& reg = my_it->second;

    bRet = reg.decreasePosRecord(who,contractId, amount, price, inverse, collateral_currency, direction);

    return bRet;
}

// Decrease Position Record
bool Register::decreasePosRecord(const std::string& who, uint32_t contractId, int64_t amount, int64_t price, bool inverse, int64_t collateral_currency, int direction)
{
    bool bRet = false;
    RecordMap::iterator it = mp_record.find(contractId);
    //add logic where if who==null then do a general sweep for the contract, but otherwise we're doing a specific position
    //the logic doesn't currently reflect this
    if (it != mp_record.end())
    {
        PositionRecord& record = it->second;
        Entries& entries = record.entries;

        // setting remaining
        int64_t& remaining = amount;

        auto itt = entries.begin();

        // The loop below used to call entries.erase(itt) and then build a
        // replacement pair from references into the erased element and
        // push_back it into the same vector (which may also reallocate),
        // without ever advancing itt otherwise: dangling references and an
        // invalidated iterator (UB), plus skipped/mis-accounted entries when
        // more than one entry had to be consumed. Use the iterator returned by
        // erase() to advance, and collect reduced entries in a separate vector
        // that is appended after the loop (same final ordering as intended:
        // a partially closed entry moves to the back).
        Entries additions;

        while(remaining > 0 && itt != entries.end())
        {
            // process to calculate it
            std::pair<int64_t,int64_t>& p = *itt;

            int64_t& ramount = p.first;
            const int64_t& rprice = p.second;

            if(remaining - ramount >= 0)
            {
                // smaller remaining
                remaining -= ramount;
                PrintToLog("%s():deleting full entry: amount: %d, price: %d\n",__func__, ramount, rprice);
                realizePNL(who, contractId, ramount,price, inverse, collateral_currency, direction);
                itt = entries.erase(itt);
            } else {
                //there's nothing left
                ramount -= remaining;
                remaining = 0;
                realizePNL(who, contractId, ramount,price, inverse, collateral_currency, direction);
                // copy the reduced entry out and log while the element is still
                // alive; the old code read ramount/rprice after erase()
                additions.push_back(std::make_pair(ramount, rprice));
                PrintToLog("%s() there's nothing else (remaining == 0), ramount left: %d, at price: %d\n",__func__, ramount, rprice);
                itt = entries.erase(itt);
            }

        }

        // re-append partially closed entries collected during the loop
        entries.insert(entries.end(), additions.begin(), additions.end());

        // closing position and then open a new one on the other side
        if (remaining > 0)
        {
            entries.clear();
            PrintToLog("%s(): closing position and then open a new one on the other side, remaining: %d, price: %d\n",__func__, remaining, price);
            std::pair<int64_t,int64_t> p (remaining, price);
            entries.push_back(p);
        }

        bRet = true;

    }


    return bRet;
}

bool Register::realizePNL(const std::string& who, uint32_t contractId, int64_t amount, int64_t price, bool isInverseQuoted, uint32_t collateral_currency, int direction)
{
            //we want to do what settlementPNL does but with actual exit prices instead of mark prices
          //this is called when the logic of xTrade or instant trade in mdex.cpp notes a reduction in position. 
          //it's probable that a version simply referring to the mark price of the last block, being parsed this block in the tx_handler, would capture any
          //excess PNL or loss since the last block would only be updating, this makes sense (vs. calculating the whole trade PNL)
      bool bRet = false;
      int64_t notionalsize = 1; //this is a temporary measure for testing, really I need to route this parameter into this function flow 
    LOCK(cs_register);
    
        // NOTE: despite its name, "entry" below is NOT the position's entry
        // price: it is the mark price of the position as of the last block
        // (getPosMarkPrice(contractId, true)), i.e. we do what settlement_pnl
        // does but with the actual exit price of this reduction ("price")
        // instead of the mark price.
        const int64_t entry = getPosMarkPrice(contractId, true);
        const int64_t exit = price;
        int64_t realizedPNL = 0;
        
        // The guard used to be "if(entry||exit==0)", which is true for every
        // non-zero entry price (zeroing the PnL in the normal case) and lets
        // entry == 0 fall through to the divisions below - an integer
        // divide-by-zero. Reject the degenerate cases instead: no entry price,
        // or a zero exit price (also a divisor).
        if(entry == 0 || exit == 0){
            realizedPNL = 0;
        }else{
            // Both formulas below are written for the LONG side (a long
            // profits when the price rises, matching the sign rule in
            // getUPNL) and are then multiplied by the direction of the
            // position being closed (direction > 0: long, < 0: short), so a
            // short close gets the mirrored PnL. Without the direction
            // multiplier the expressions were direction-blind and e.g. a
            // profitable long close on an inverse contract (entry = 2e8,
            // exit = 3e8) came out as (1/exit - 1/entry) * notional < 0.
            if(isInverseQuoted){
                // Long-side realized PnL over the notional for inverse-quoted
                // contracts, in COIN-scaled fixed point:
                // (1/entry - 1/exit) * notionalsize. With COIN-scaled prices
                // the COIN-scaled reciprocal of a price is COIN*COIN/price,
                // hence the expression below. (The first fixed version had
                // "1/exit - 1/entry", the short-side convention, which made a
                // profitable long close negative.)
                realizedPNL = ((COIN * COIN) / entry - (COIN * COIN) / exit) * notionalsize;
            }else{
                // Long-side realized PnL over the notional for linear
                // contracts: (exit - entry) * notionalsize. The old precedence
                // computed exit - (entry * notionalsize).
                realizedPNL = (exit - entry) * notionalsize;
            }

            // "direction" is the sign of the position this reduction closes,
            // threaded down from updateAllEntry()'s signOld (the entry amounts
            // consumed here are unsigned, so the direction cannot be recovered
            // locally: insert_entry() always receives a positive count).
            if (direction < 0) {
                realizedPNL = -realizedPNL;
            }
        }

        //PrintToLog("%s(): upnl: %d, oldPNL: %d, newUPNL: %d\n",__func__, upnl, oldPNL, newUPNL);

        if (0 != realizedPNL)
        {
            PrintToLog("%s(): updating register map (because newUPNL is not zero)\n",__func__);

            update_register_map(who, contractId, realizedPNL, MARGIN);
            update_register_map(who, contractId, realizedPNL, PNL);
            update_tally_map(who, collateral_currency, realizedPNL, BALANCE);
            bRet = true;
        }

    return bRet;
}
