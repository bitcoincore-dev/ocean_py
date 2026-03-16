#!/usr/bin/env python3
import requests
import sys

def get_ocean_health():
    slug = "ocean"
    url = f"https://mempool.space/api/v1/mining/pool/{slug}"
    blocks_url = f"https://mempool.space/api/v1/mining/pool/{slug}/blocks"

    try:
        # 1. Get Aggregate Pool Health
        pool_res = requests.get(url, timeout=10)
        pool_res.raise_for_status()
        pool_data = pool_res.json()

        avg_health = pool_data.get("avgBlockHealth", "N/A")

        # 2. Get Individual Block Health (Match Rate)
        block_res = requests.get(blocks_url, timeout=10)
        block_res.raise_for_status()
        blocks = block_res.json()

        print("--- OCEAN Pool Health Metrics ---")
        print(f"Aggregate Pool Health (avgBlockHealth): {avg_health}%")
        print("\n--- Recent Block Health (matchRate) ---")

        for b in blocks[:10000]: # Show last N blocks
            height = b.get("height")
            # matchRate is nested in 'extras'
            match_rate = b.get("extras", {}).get("matchRate", "N/A")
            print(f"Block Height: {height} | Health: {match_rate}%")

    except Exception as e:
        print(f"Error fetching metrics: {e}")
        sys.exit(1)

if __name__ == "__main__":
    get_ocean_health()
