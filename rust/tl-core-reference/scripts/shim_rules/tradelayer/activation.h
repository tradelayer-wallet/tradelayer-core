// Stand-in for tradelayer/activation.h: records AddPendingActivation calls with the same
// delete-then-push semantics as the real activation.cpp so the harness can print them.
#pragma once
#include <stdint.h>
#include <string>
#include <vector>

struct ShimPending {
    uint16_t featureId;
    int activationBlock;
    uint32_t minClientVersion;
};
inline std::vector<ShimPending>& shim_pending()
{
    static std::vector<ShimPending> v;
    return v;
}
namespace mastercore {
inline void AddPendingActivation(uint16_t featureId, int activationBlock, uint32_t minClientVersion, const std::string&)
{
    std::vector<ShimPending>& v = shim_pending();
    for (std::vector<ShimPending>::iterator it = v.begin(); it != v.end();)
        (it->featureId == featureId) ? it = v.erase(it) : ++it;
    v.push_back({featureId, activationBlock, minClientVersion});
}
} // namespace mastercore
