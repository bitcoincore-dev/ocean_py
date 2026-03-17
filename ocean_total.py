#!/usr/bin/env python3
import requests
import sys
import time
import json

def fetch_all_ocean_blocks():
    slug = "ocean"
    base_blocks_url = f"https://mempool.space/api/v1/mining/pool/{slug}/blocks"
    price_url = "https://mempool.space/api/v1/prices"

    all_blocks = []
    last_height = None

    try:
        # 1. Get current BTC Price
        btc_usd = requests.get(price_url, timeout=10).json().get("USD", 0)

        print(f"--- Starting Full History Crawl for OCEAN (Price: ${btc_usd:,.2f}) ---")

        while True:
            # Construct URL with height offset for pagination
            url = f"{base_blocks_url}/{last_height}" if last_height else base_blocks_url

            response = requests.get(url, timeout=10)
            response.raise_for_status()
            batch = response.json()

            if not batch:
                break

            all_blocks.extend(batch)
            last_height = batch[-1]['height']

            print(f"Fetched {len(all_blocks)} blocks... (Current Height: {last_height})")

            # Respect API rate limits
            time.sleep(0.5)

        # 2. Process and Calculate
        total_loss_usd = 0
        processed_data = []

        print(f"\n{'Height':<10} | {'Health':<8} | {'Loss (Sats)':<12} | {'Loss (USD)':<10}")
        print("-" * 50)

        for b in all_blocks:
            extras = b.get("extras", {})
            match_rate = extras.get("matchRate", 0)
            actual_reward = extras.get("reward", 0)

            if match_rate > 0 and match_rate < 100:
                expected_reward = int(actual_reward / (match_rate / 100))
                loss_sats = expected_reward - actual_reward
            else:
                loss_sats = 0

            loss_usd = (loss_sats / 100_000_000) * btc_usd
            total_loss_usd += loss_usd

            processed_data.append({
                "height": b['height'],
                "health": match_rate,
                "loss_sats": loss_sats,
                "loss_usd": round(loss_usd, 2)
            })

            # Print a sample of the first few
            if len(processed_data) <= 1000:
                print(f"{b['height']:<10} | {match_rate:>6.2f}% | {loss_sats:>12,} | ${loss_usd:>8.2f}")

        # 3. Output Summary
        print("-" * 50)
        print(f"TOTAL BLOCKS MINED: {len(all_blocks)}")
        print(f"TOTAL CUMULATIVE LOSS: ${total_loss_usd:,.2f}")

        # Save to file
        with open("ocean_full_history.json", "w") as f:
            json.dump(processed_data, f, indent=4)
        print(f"\nFull dataset saved to: ocean_full_history.json")

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    fetch_all_ocean_blocks()
