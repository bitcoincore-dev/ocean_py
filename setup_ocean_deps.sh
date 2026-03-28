#!/bin/bash

# This script installs OS-specific dependencies and compiles BIP-64MOD.
# It should be run manually by the user.

OS=$(uname -s | tr '[:upper:]' '[:lower:]')

echo "--- Installing OS-specific dependencies (Detected OS: $OS) ---"

if [ "$OS" == "linux" ]; then
    echo "Installing build essentials and GCC for Linux..."
    sudo apt-get update || exit 1
    sudo apt-get install -y build-essential gcc g++ autoconf automake libtool 
        pkg-config libssl-dev libevent-dev bsdmainutils libboost-system-dev 
        libboost-filesystem-dev libboost-chrono-dev libboost-test-dev 
        libboost-thread-dev || exit 1
elif [ "$OS" == "darwin" ]; then # macOS
    echo "Checking for Homebrew and dependencies..."
    if command -v brew >/dev/null; then
        brew install gcc boost openssl libevent autoconf automake || exit 1
    else
        echo "Error: Homebrew not found. Please install it or ensure GCC is available via Xcode."
        exit 1
    fi
else
    echo "Warning: Unsupported operating system: $OS. Please install dependencies manually."
fi

echo "
--- Compiling with GCC ---"
# Using 'gcc' - note that on macOS 'gcc' is often a symlink to 'clang'
gcc -O2 -Wall -c -fPIC bip64mod_config.h -o bip64mod.o || exit 1

echo "BIP-64MOD compilation complete."
