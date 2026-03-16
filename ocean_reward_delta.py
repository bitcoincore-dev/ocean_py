#!/usr/bin/env python3
import requests
import sys

def get_ocean_health():
    slug = "ocean"
    pool_url = f"https://mempool.space/api/v1/mining/pool/{slug}"
    blocks_url = f"https://mempool.space/api/v1/mining/pool/{slug}/blocks"
    price_url = "https://mempool.space/api/v1/prices"

    try:
        # 1. Fetch Current BTC Price (USD)
        price_res = requests.get(price_url, timeout=10)
        price_res.raise_for_status()
        btc_usd = price_res.json().get("USD")

        # 2. Get Aggregate Pool Health
        pool_res = requests.get(pool_url, timeout=10)
        pool_res.raise_for_status()
        pool_data = pool_res.json()
        avg_health = pool_data.get("avgBlockHealth", "N/A")

        # 3. Get Individual Block Data
        block_res = requests.get(blocks_url, timeout=10)
        block_res.raise_for_status()
        blocks = block_res.json()

        print(f"--- OCEAN Pool Metrics (BTC Price: ${btc_usd:,.2f}) ---")
        print(f"Aggregate Pool Health: {avg_health}%")
        print("\n" + "-" * 90)
        print(f"{'Height':<10} | {'Health':<8} | {'Actual (Sats)':<15} | {'Expected (Sats)':<15} | {'Loss (USD)':<10}")
        print("-" * 90)

        for b in blocks[:10]: # Limiting display to 10 for readability
            height = b.get("height")
            extras = b.get("extras", {})

            match_rate = extras.get("matchRate", 0)
            actual_reward = extras.get("reward", 0)
            # Calculate Expected: Actual Reward / (Match Rate / 100)
            # This accounts for the fees missed due to template mismatch
            if match_rate > 0:
                expected_reward = int(actual_reward / (match_rate / 100))
            else:
                expected_reward = actual_reward

            diff_sats = expected_reward - actual_reward
            diff_usd = (diff_sats / 100_000_000) * btc_usd

            print(f"{height:<10} | {match_rate:>6.2f}% | {actual_reward:>15,} | {expected_reward:>15,} | ${diff_usd:>8.2f}")

    except Exception as e:
        print(f"Error fetching metrics: {e}")
        sys.exit(1)

if __name__ == "__main__":
    # Use the interpreter from the venv if available
    get_ocean_health()
