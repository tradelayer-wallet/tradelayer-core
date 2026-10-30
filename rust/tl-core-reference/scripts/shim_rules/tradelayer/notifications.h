// Stand-in for tradelayer/notifications.h: counts alerts instead of raising them.
#pragma once
#include <stdint.h>
#include <string>
#define ALERT_BLOCK_EXPIRY 1
inline int& shim_alert_count()
{
    static int n = 0;
    return n;
}
inline void AddAlert(const std::string&, uint16_t, uint32_t, const std::string&) { ++shim_alert_count(); }
inline void DoWarning(const std::string&) {}
