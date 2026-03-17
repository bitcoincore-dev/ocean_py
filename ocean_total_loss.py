#!/usr/bin/env python3
import requests
import sys
import time
import json
from tqdm import tqdm  # Run: pip install tqdm

MIRRORS = [
    "https://mempool.space",
    "https://mempool.sweetsats.io"
]

def fetch_with_failover(path, timeout=10):
    for base_url in MIRRORS:
        url = f"{base_url}{path}"
        try:
            response = requests.get(url, timeout=timeout)
            if response.status_code == 200:
                return response.json(), base_url
            if response.status_code == 429:
                continue
        except Exception:
            continue
    return None, None

def get_pool_stats():
    """Fetches total block count for OCEAN to initialize progress bar."""
    data, _ = fetch_with_failover("/api/v1/mining/pool/ocean")
    if data:
        # Pulling the 'All' block count from pool stats
        return data.get("pool_stats", {}).get("blockCount", 832)
    return 832

def fetch_full_ocean_report():
    slug = "ocean"
    all_blocks = []
    last_height = ""

    total_expected = get_pool_stats()

    print(f"--- OCEAN History Audit ---")
    print(f"Total Blocks Expected: {total_expected}")

    # Initialize Progress Bar
    pbar = tqdm(total=total_expected, desc="Crawling Blocks", unit="blk")

    while True:
        path = f"/api/v1/mining/pool/{slug}/blocks/{last_height}"
        batch, active_mirror = fetch_with_failover(path)

        if not batch:
            pbar.write("Done: Reached the end of the block chain.")
            break

        all_blocks.extend(batch)
        last_height = batch[-1]['height']

        pbar.update(len(batch))
        time.sleep(0.3)

    pbar.close()

    # Processing with Historical Prices
    total_loss_usd = 0.0
    processed_data = []

    print(f"\n{'Height':<10} | {'Match Rate':<10} | {'Loss (USD)':<10}")
    print("-" * 40)

    # Re-using a progress bar for the historical price lookups (the slow part)
    for b in tqdm(all_blocks, desc="Calculating Loss (USD)", unit="blk"):
        ts = b.get('timestamp')
        extras = b.get("extras", {}) or {}
        match_rate = extras.get("matchRate")
        actual_reward = extras.get("reward", 0)

        if match_rate is not None and 0 < match_rate < 100:
            expected_reward = int(actual_reward / (match_rate / 100))
            loss_sats = expected_reward - actual_reward
        else:
            loss_sats = 0
            if match_rate is None: match_rate = 100.0

        # Failover historical price lookup
        price_path = f"/api/v1/historical-price?timestamp={ts}&currency=USD"
        price_data, _ = fetch_with_failover(price_path, timeout=5)
        hist_price = price_data.get('usd', 0) if price_data else 74000.0

        loss_usd = (loss_sats / 100_000_000) * hist_price
        total_loss_usd += loss_usd

        processed_data.append({
            "height": b['height'],
            "match_rate": match_rate,
            "loss_usd": round(loss_usd, 2)
        })

    print("-" * 40)
    print(f"TOTAL BLOCKS: {len(processed_data)}")
    print(f"TOTAL LOSS:  ${total_loss_usd:,.2f}")

    with open("ocean_historical_report.json", "w") as f:
        json.dump(processed_data, f, indent=4)

if __name__ == "__main__":
    fetch_full_ocean_report()
