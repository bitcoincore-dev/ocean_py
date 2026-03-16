#!/usr/bin/env python3
import os
import subprocess
import sys
import platform

def run_command(command, shell=True):
    """Helper to run shell commands and exit on failure."""
    try:
        print(f"Executing: {command}")
        # On Windows, shell=True is generally needed for built-ins
        # On Unix, it allows string commands with pipes/redirects
        subprocess.run(command, shell=shell, check=True)
    except subprocess.CalledProcessError as e:
        print(f"Error executing command: {e}")
        sys.exit(1)

def main():
    # 1. Environment Variables & Constants
    os.environ["POOL_URL"] = "mine.ocean.xyz:3334"
    os.environ["USER_ADDRESS"] = "YOUR_BITCOIN_ADDRESS"
    os.environ["WORKER_NAME"] = "gcc_node_01"

    # Cross-platform thread detection
    build_threads = os.cpu_count() or 1

    print(f"Starting configuration for OCEAN Mining Pool (Detected OS: {platform.system()})...")

    # 2. Dependency Installation (OS Specific)
    current_os = platform.system().lower()

    if current_os == "linux":
        print("Installing build essentials and GCC for Linux...")
        run_command("sudo apt-get update")
        run_command("sudo apt-get install -y build-essential gcc g++ autoconf automake libtool "
                    "pkg-config libssl-dev libevent-dev bsdmainutils libboost-system-dev "
                    "libboost-filesystem-dev libboost-chrono-dev libboost-test-dev "
                    "libboost-thread-dev")
    elif current_os == "darwin":  # macOS (Your current system)
        print("Checking for Homebrew and dependencies...")
        # Check if brew is installed first
        if subprocess.run("command -v brew", shell=True, capture_output=True).returncode == 0:
            run_command("brew install gcc boost openssl libevent autoconf automake")
        else:
            print("Homebrew not found. Please install it or ensure GCC is available via Xcode.")

    # 3. Apply BIP-64MOD Logic (File Creation)
    print("Applying BIP-64MOD protocol extensions...")
    bip64_content = """/* BIP-64MOD + GCC Integration Header */
#define BIP64_MOD_ENABLED 1
#define OCEAN_TIDES_SUPPORT 1
#define MAX_METADATA_PEERS 128

typedef struct {
    char peer_addr[64];
    uint32_t version_mod;
    uint64_t session_id;
} BIP64ModContext;
"""
    with open("bip64mod_config.h", "w") as f:
        f.write(bip64_content)

    # 4. Compile Integration Module
    print("Compiling with GCC...")
    # Using 'gcc' - note that on macOS 'gcc' is often a symlink to 'clang'
    run_command("gcc -O2 -Wall -c -fPIC bip64mod_config.h -o bip64mod.o")

    # 5. OCEAN Node Policy Flags (Append to bitcoin.conf)
    print("Generating bitcoin.conf recommended flags for OCEAN...")
    conf_content = f"""
# OCEAN recommended node policy
blockmaxsize=3985000
blockmaxweight=3985000
mempoolfullrbf=1
permitbaremultisig=0
datacarriersize=42
# BIP-64MOD specific relay settings
bip64mod=1
"""
    # Open in 'a+' to create if doesn't exist, otherwise append
    with open("bitcoin.conf", "a+") as f:
        f.write(conf_content)

    print("-" * 55)
    print("Setup Complete.")
    print(f"Pool: {os.environ['POOL_URL']}")
    print(f"Username: {os.environ['USER_ADDRESS']}.{os.environ['WORKER_NAME']}")
    print(f"Threads: {build_threads}")
    print("BIP-64MOD context has been preserved in bip64mod_config.h")
    print("-" * 55)

if __name__ == "__main__":
    main()
