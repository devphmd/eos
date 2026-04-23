#pragma once

// Core platform + interface getters / tick
#include "eos_sdk.h"

// Function APIs we want available in Rust (not just *_types.h)
#include "eos_auth.h"
#include "eos_connect.h"
#include "eos_lobby.h"
#include "eos_p2p.h"
#include "eos_sessions.h"

