// Stand-in for tradelayer/log.h: logging is discarded, debug switches are off.
#pragma once
#include <string>
template <typename... A>
void PrintToLog(const char*, const A&...) {}
// rules.cpp builds alert text with strprintf; the text is not observable here.
template <typename... A>
std::string strprintf(const char* fmt, const A&...) { return fmt; }
static bool msc_debug_activate_feature = false;
static bool msc_debug_deactivate_feature = false;
static bool msc_debug_is_transaction_type_allowed = false;
