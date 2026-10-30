// Differential vector generator for tl-core-reference.
//
// Compiles the UNMODIFIED src/tradelayer/createpayload.cpp and src/tradelayer/varint.cpp
// (two empty stub headers in scripts/shim satisfy their unused includes) and prints
// "name<TAB>hex" lines for fixed inputs. See scripts/build-vectors.sh.
#include "../../../src/tradelayer/createpayload.cpp"
#include "../../../src/tradelayer/varint.cpp"

#include <cstdio>
#include <string>
#include <vector>

typedef std::vector<unsigned char> Bytes;

static void emit(const char* name, const Bytes& b)
{
    std::printf("%s\t", name);
    for (unsigned char c : b) std::printf("%02x", c);
    std::printf("\n");
}

static void emit_dec(const char* name, const Bytes& b)
{
    std::printf("%s\t%llu\n", name, (unsigned long long)DecompressInteger(b));
}

int main()
{
    // ---- varint -------------------------------------------------------------
    const uint64_t vals[] = {0, 1, 127, 128, 255, 300, 16383, 16384, 4294967295ULL, 4294967296ULL,
                             9223372036854775808ULL, 18446744073709551615ULL};
    for (uint64_t v : vals) {
        char n[64];
        std::snprintf(n, sizeof n, "varint/%llu", (unsigned long long)v);
        emit(n, CompressInteger(v));
    }
    // Lenient decoder behaviour on non-canonical / odd inputs (decimal result).
    emit_dec("varint_decompress/80_00", Bytes{0x80, 0x00});
    emit_dec("varint_decompress/ff_00", Bytes{0xff, 0x00});
    emit_dec("varint_decompress/ff_ff_7f", Bytes{0xff, 0xff, 0x7f});
    emit_dec("varint_decompress/max10", Bytes{0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x01});
    emit_dec("varint_decompress/ten_high", Bytes{0x80,0x80,0x80,0x80,0x80,0x80,0x80,0x80,0x80,0x7f});

    // ---- payloads -----------------------------------------------------------
    emit("simple_send/basic", CreatePayload_SimpleSend(7, 100000000ULL));
    emit("simple_send/zero", CreatePayload_SimpleSend(0, 0));
    emit("simple_send/max", CreatePayload_SimpleSend(4294967295U, 18446744073709551615ULL));

    emit("send_many/basic", CreatePayload_SendMany(3, std::vector<uint64_t>{1, 2, 300}));
    emit("send_many/empty", CreatePayload_SendMany(3, std::vector<uint64_t>{}));
    emit("send_many/zero_amount", CreatePayload_SendMany(3, std::vector<uint64_t>{5, 0, 7}));

    emit("send_vesting/basic", CreatePayload_SendVestingTokens(123456789ULL));
    emit("send_all/basic", CreatePayload_SendAll());

    {
        std::string name = "TOK", url = "https://example.invalid", data = "meta";
        std::vector<int> kyc = {0, 1, 2};
        emit("issuance_fixed/basic", CreatePayload_IssuanceFixed(2, 0, name, url, data, 21000000ULL, kyc));
    }
    {
        std::string name = "T", url = "", data = "";
        std::vector<int> kyc;
        emit("issuance_fixed/nokyc", CreatePayload_IssuanceFixed(1, 5, name, url, data, 1, kyc));
    }
    {
        std::string name(300, 'a'), url(256, 'b'), data(255, 'c');
        std::vector<int> kyc = {7};
        emit("issuance_fixed/long_strings", CreatePayload_IssuanceFixed(2, 0, name, url, data, 5, kyc));
    }
    {
        std::string name = "N", url = "u", data = "d";
        std::vector<int> kyc = {-1};
        emit("issuance_fixed/negative_kyc", CreatePayload_IssuanceFixed(2, 0, name, url, data, 5, kyc));
    }
    {
        std::string name = "MAN", url = "https://example.invalid/m", data = "managed";
        std::vector<int> kyc = {3, 4};
        emit("issuance_managed/basic", CreatePayload_IssuanceManaged(2, 0, name, url, data, kyc));
    }
    emit("grant/basic", CreatePayload_Grant(5, 1000));
    emit("revoke/basic", CreatePayload_Revoke(5, 500));
    emit("change_issuer/basic", CreatePayload_ChangeIssuer(9));

    emit("deactivate_feature/basic", CreatePayload_DeactivateFeature(8));
    emit("activate_feature/basic", CreatePayload_ActivateFeature(8, 400, 1));
    emit("activate_feature/max", CreatePayload_ActivateFeature(28, 4294967295U, 4294967295U));
    emit("alert/basic", CreatePayload_TradeLayerAlert(1, 1000, "hello"));
    emit("alert/clear", CreatePayload_TradeLayerAlert(65535, 0, ""));

    {
        std::string name = "ALL-LTC";
        std::vector<int> kyc = {0, 2};
        emit("create_contract/basic", CreatePayload_CreateContract(1, 1, name, 0, 1, 1, 100000000ULL, 1, kyc));
    }
    {
        std::string name = "BTC-USD";
        std::vector<int> kyc;
        emit("create_contract/expiring", CreatePayload_CreateContract(3, 2, name, 4032, 10, 4, 250000ULL, 0, kyc));
    }
    {
        std::string name = "ALL-LTC";
        emit("contract_dex_trade/basic", CreatePayload_ContractDexTrade(name, 100, 25000000ULL, 1, 10));
    }
    emit("contract_dex_cancel_all/basic", CreatePayload_ContractDexCancelAll(3));
    emit("contract_dex_cancel_by_txid/basic", CreatePayload_ContractDexCancelOrderByTxId(1234, 5));
    emit("contract_dex_cancel_by_txid/negative_block", CreatePayload_ContractDexCancelOrderByTxId(-1, 0));
    emit("contract_dex_close_position/basic", CreatePayload_ContractDexClosePosition(3));

    {
        std::string name = "sLTC";
        emit("issuance_pegged/basic", CreatePayload_IssuancePegged(8, 0, name, 5, 3, 1000));
    }
    emit("redemption_pegged/basic", CreatePayload_RedemptionPegged(5, 3, 1000));
    emit("send_pegged/basic", CreatePayload_SendPeggedCurrency(5, 1000));

    emit("dex_sell/basic", CreatePayload_DExSell(4, 100000000ULL, 5000000ULL, 10, 1000, 1));
    emit("dex_buy/basic", CreatePayload_DEx(4, 100000000ULL, 123456ULL, 10, 1000, 2));
    emit("dex_accept/basic", CreatePayload_DExAccept(4, 777));

    emit("metadex_trade/basic", CreatePayload_MetaDExTrade(4, 1000, 5, 2000));
    {
        int64_t a = 1000, b = 2000;
        emit("metadex_cancel_price/basic", CreatePayload_MetaDExCancelPrice(4, a, 5, b));
        int64_t na = -1;
        emit("metadex_cancel_price/negative", CreatePayload_MetaDExCancelPrice(4, na, 5, b));
    }
    emit("metadex_cancel_pair/basic", CreatePayload_MetaDExCancelPair(4, 5));
    emit("metadex_cancel_all/basic", CreatePayload_MetaDExCancelAll());

    {
        std::string name = "ORACLE1";
        std::vector<int> kyc = {1};
        emit("create_oracle_contract/basic", CreatePayload_CreateOracleContract(name, 0, 1, 1, 100000ULL, 1, kyc));
    }
    emit("change_oracle_adm/basic", CreatePayload_Change_OracleAdm(3));
    emit("set_oracle/basic", CreatePayload_Set_Oracle(3, 30000, 29000, 29500));
    emit("oracle_backup/basic", CreatePayload_OracleBackup(3));
    emit("close_oracle/basic", CreatePayload_Close_Oracle(3));

    emit("commit_channel/basic", CreatePayload_Commit_Channel(4, 5000));
    emit("withdrawal_from_channel/basic", CreatePayload_Withdrawal_FromChannel(4, 5000));
    emit("instant_trade/basic", CreatePayload_Instant_Trade(4, 100, 5000, 5, 200));
    emit("instant_trade/negative_expiry", CreatePayload_Instant_Trade(4, 100, -1, 5, 200));
    emit("contract_instant_trade/basic", CreatePayload_Contract_Instant_Trade(3, 10, 5000, 25000000ULL, 1, 5));
    emit("pnl_update/basic", CreatePayload_PNL_Update(4, 300, 5000));
    emit("transfer/basic", CreatePayload_Transfer(1, 4, 250));
    emit("instant_ltc_trade/basic", CreatePayload_Instant_LTC_Trade(4, 100, 12345, 5000));

    {
        std::string site = "example.invalid", company = "Example Co";
        emit("new_id_registration/basic", CreatePayload_New_Id_Registration(site, company));
    }
    emit("update_id_registration/basic", CreatePayload_Update_Id_Registration());
    emit("dex_payment/basic", CreatePayload_DEx_Payment());
    {
        std::string h = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        emit("attestation/basic", CreatePayload_Attestation(h));
        emit("contract_dex_cancel/basic", CreatePayload_ContractDExCancel(h));
        emit("dex_cancel/basic", CreatePayload_DExCancel(h));
    }
    emit("revoke_attestation/basic", CreatePayload_Revoke_Attestation());
    emit("close_channel/basic", CreatePayload_Close_Channel());
    emit("submit_node_address/basic", CreatePayload_SubmitNodeAddress());
    emit("claim_node_reward/basic", CreatePayload_ClaimNodeReward());
    emit("send_donation/basic", CreatePayload_SendDonation(4, 42));
    return 0;
}
