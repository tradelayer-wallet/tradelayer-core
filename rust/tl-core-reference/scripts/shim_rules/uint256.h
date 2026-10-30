// Stand-in for Bitcoin Core's uint256.h: rules.cpp only needs the type to exist and compare.
#pragma once
#include <string>
struct uint256 {
    std::string GetHex() const { return ""; }
    bool operator!=(const uint256&) const { return false; }
};
