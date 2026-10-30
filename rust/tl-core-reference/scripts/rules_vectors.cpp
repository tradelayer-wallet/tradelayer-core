// Differential vector generator for the activation rules in tl-core-reference.
//
// Compiles the UNMODIFIED src/tradelayer/rules.cpp against small stand-in headers in
// scripts/shim_rules (logging, alerts, chain params) and prints "key<TAB>value" lines.
// See scripts/build-vectors.sh.
#include "../../../src/tradelayer/rules.cpp"

#include <cstdio>
#include <string>
#include <vector>

using namespace mastercore;

#define FIELDS(X) \
    X(GENESIS_BLOCK) X(MULTISIG_BLOCK) X(MIN_ACTIVATION_BLOCKS) X(MAX_ACTIVATION_BLOCKS) \
    X(PUBKEYHASH_BLOCK) X(SCRIPTHASH_BLOCK) X(NULLDATA_BLOCK) X(MSC_ALERT_BLOCK) X(MSC_SEND_BLOCK) \
    X(MSC_SEND_MANY_BLOCK) X(MSC_SP_BLOCK) X(MSC_MANUALSP_BLOCK) X(MSC_SEND_ALL_BLOCK) \
    X(INFLEXION_BLOCK) X(MSC_VESTING_CREATION_BLOCK) X(MSC_VESTING_BLOCK) X(MSC_PEGGED_CURRENCY_BLOCK) \
    X(MSC_KYC_BLOCK) X(MSC_METADEX_BLOCK) X(MSC_DEXSELL_BLOCK) X(MSC_DEXBUY_BLOCK) \
    X(MSC_CONTRACTDEX_BLOCK) X(MSC_CONTRACTDEX_ORACLES_BLOCK) X(MSC_NODE_REWARD_BLOCK) \
    X(MSC_TRADECHANNEL_TOKENS_BLOCK) X(MSC_TRADECHANNEL_CONTRACTS_BLOCK) X(MSC_TRADECHANNEL_OPTIONS_BLOCK) \
    X(MSC_DISPENSERVAULTS_BLOCK) X(MSC_PAYMENTBATCHING_BLOCK) X(MSC_MARGINLENDING_BLOCK) \
    X(MSC_INTEROP_CTV_BLOCK) X(MSC_INTEROP_LIGHTNING_BLOCK) X(MSC_INTEROP_REPO_BLOCK) \
    X(MSC_INTEROP_SIDECHAINS_BLOCK) X(MSC_INTEROP_CROSSCHAINATOMICSWAPS_BLOCK) \
    X(MSC_GRAPHDEFAULTSWAPS_BLOCK) X(MSC_INTERESTRATESWAPS_BLOCK) X(MSC_MINERFEECONTRACTS_BLOCK) \
    X(MSC_MASSPAYMENT_BLOCK) X(MSC_MULTISEND_BLOCK) X(MSC_HEDGEDCURRENCY_BLOCK) \
    X(MSC_SEND_DONATION_BLOCK) X(ONE_YEAR)

static const char* NETS[] = {"main", "test", "regtest"};

static std::string dump_fields(const CConsensusParams& p)
{
    std::string s;
#define X(f) s += (s.empty() ? "" : ","); s += std::to_string(p.f);
    FIELDS(X)
#undef X
    return s;
}

static std::string dump_pending()
{
    std::string s;
    for (const ShimPending& p : shim_pending()) {
        s += (s.empty() ? "" : "|");
        s += std::to_string(p.featureId) + ":" + std::to_string(p.activationBlock) + ":" + std::to_string(p.minClientVersion);
    }
    return s;
}

static void reset(const char* net)
{
    shim_network() = net;
    ResetConsensusParams();
    shim_pending().clear();
    shim_alert_count() = 0;
}

static void state(const std::string& key, bool ret)
{
    std::printf("%s\tret=%d;alerts=%d;pending=%s;params=%s\n", key.c_str(), ret ? 1 : 0, shim_alert_count(),
                dump_pending().c_str(), dump_fields(ConsensusParams()).c_str());
}

int main()
{
    const int blocks[] = {0, 1, 99, 100, 200, 201, 1000, 2121694, 2121695, 2487299, 2487300, 2490000, 99999998, 99999999, 100000000};
    const unsigned types[] = {0, 1, 2, 4, 5, 10, 11, 20, 21, 22, 25, 26, 29, 30, 31, 32, 33, 34, 35, 36, 37, 40, 50, 54, 55, 56, 70,
                              100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119,
                              120, 121, 122, 123, 3, 99, 124, 65532, 65533, 65534, 65535};
    const unsigned versions[] = {0, 1, 2, 65535};

    for (const char* net : NETS) {
        reset(net);
        std::printf("params/%s\t%s\n", net, dump_fields(ConsensusParams()).c_str());

        // transaction restriction table (resolved to numbers)
        const std::vector<TransactionRestriction> r = ConsensusParams().GetRestrictions();
        for (size_t i = 0; i < r.size(); ++i)
            std::printf("restriction/%s/%zu\t%u,%u,%d,%d\n", net, i, (unsigned)r[i].txType, (unsigned)r[i].txVersion,
                        r[i].allowWildcard ? 1 : 0, r[i].activationBlock);

        for (unsigned f = 0; f <= 31; ++f) {
            std::printf("feature_name/%u\t%s\n", f, GetFeatureName((uint16_t)f).c_str());
            for (int b : blocks)
                std::printf("is_feature_activated/%s/f%u/b%d\t%d\n", net, f, b, IsFeatureActivated((uint16_t)f, b) ? 1 : 0);
        }
        for (unsigned t : types)
            for (unsigned v : versions)
                for (int b : blocks)
                    std::printf("is_tx_allowed/%s/t%u/v%u/b%d\t%d\n", net, t, v, b, IsTransactionTypeAllowed(b, (uint16_t)t, (uint16_t)v) ? 1 : 0);
    }

    // ---- ActivateFeature / DeactivateFeature scenarios -------------------------------------
    // key: scn/<name>/<step>; value: ret, alert count, pending list, every consensus field.
    {
        reset("regtest"); // MIN_ACTIVATION_BLOCKS=5, MAX_ACTIVATION_BLOCKS=1000, MSC_SP_BLOCK=200
        int k = 0;
        auto A = [&](const char* sc, uint16_t id, int blk, uint32_t minv, int txb) {
            state(std::string("scn/") + sc + "/" + std::to_string(k++), ActivateFeature(id, blk, minv, txb));
        };
        auto D = [&](const char* sc, uint16_t id, int txb) {
            state(std::string("scn/") + sc + "/" + std::to_string(k++), DeactivateFeature(id, txb));
        };
        // notice window, already-live and ordinary activation on feature 8 (FIXED, SP_BLOCK)
        A("regtest_fixed", 8, 104, 1, 100);       // below tx+MIN -> refused
        A("regtest_fixed", 8, 1101, 1, 100);      // above tx+MAX -> refused
        A("regtest_fixed", 8, 150, 1, 100);       // already scheduled at 200 but not live at 100: accepted, reschedules
        A("regtest_fixed", 8, 400, 1, 250);       // 150 <= 250 => live -> refused
        D("regtest_fixed", 8, 100);               // not live at 100? SP_BLOCK=150 -> refused
        D("regtest_fixed", 8, 250);               // live -> deactivated, alert raised
        D("regtest_fixed", 8, 250);               // no longer live -> refused
        A("regtest_fixed", 8, 300, 1, 250);       // reactivate
        // unknown feature ids are accepted-but-unsupported by the C++ (pending activation + alert)
        A("regtest_fixed", 99, 300, 1, 250);
        A("regtest_fixed", 0, 300, 1, 250);
        D("regtest_fixed", 99, 250);              // deactivation of unknown id -> refused
        // client-version gating: supported iff TL_VERSION >= minClientVersion (TL_VERSION=20002000)
        A("regtest_fixed", 14, 300, 20002000, 250);
        A("regtest_fixed", 15, 300, 20002001, 250);
        A("regtest_fixed", 16, 300, 4294967295U, 250);

        // aliasing quirks: IsFeatureActivated(12) reads MSC_NODE_REWARD_BLOCK, (28) reads MSC_VESTING_BLOCK
        reset("regtest");
        k = 0;
        D("regtest_alias", 12, 250);   // CONTRACTDEX written block is 0, read block is NODE_REWARD (200): live at 250
        A("regtest_alias", 12, 400, 1, 250);
        D("regtest_alias", 28, 150);   // PEGGED read block is VESTING (100): live at 150
        A("regtest_alias", 28, 600, 1, 150);

        // mainnet: MIN=0, MAX=99999999
        reset("main");
        k = 0;
        A("main_window", 5, 2487299, 1, 2487300);     // activation < tx block -> refused (MIN = 0)
        A("main_window", 5, 2487300, 1, 2487300);     // == tx block, METADEX live at 2487300 -> refused (already live)
        A("main_window", 14, 2487300, 1, 2487300);    // options (never) -> ok
        A("main_window", 14, 99999999 + 2487300 + 1, 1, 2487300); // above tx+MAX -> refused
        A("main_window", 14, -5, 1, 2487300);         // negative (e.g. uint32 > INT_MAX cast) -> refused
        D("main_window", 14, 2487300);                // scheduled at tx block: live at tx block -> deactivated
    }
    return 0;
}
