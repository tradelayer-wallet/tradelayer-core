// Stand-in for chainparams.h: the harness picks the network by setting shim_network().
#pragma once
#include <string>
inline std::string& shim_network()
{
    static std::string n = "main";
    return n;
}
struct ShimChainParams {
    const std::string& NetworkIDString() const { return shim_network(); }
};
inline const ShimChainParams& Params()
{
    static ShimChainParams p;
    return p;
}
