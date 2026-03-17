#!/usr/bin/env python3
import requests
import sys
import time
import json
from tqdm import tqdm
from concurrent.futures import ThreadPoolExecutor, as_completed

MIRRORS = [
    "https://mempool.space",
    "https://mempool.sweetsats.io"
]

def fetch_worker(args):
    """Worker function for parallel execution."""
    path, index = args
    # Rotate primary mirror based on index to distribute load
    mirrors_rotated = MIRRORS[index % len(MIRRORS):] + MIRRORS[:index % len(MIRRORS)]

    for base_url in mirrors_rotated:
        url = f"{base_url}{path}"
        try:
            response = requests.get(url, timeout=10)
            if response.status_code == 200:
                return response.json()
        except:
            continue
    return None

def get_pool_stats():
    res = fetch_worker(("/api/v1/mining/pool/ocean", 0))
    return res.get("pool_stats", {}).get("blockCount", 832) if res else 832

def process_block(b, index):
    """Logic to calculate loss for a single block."""
    ts = b.get('timestamp')
    extras = b.get("extras", {}) or {}
    match_rate = extras.get("matchRate")
    actual_reward = extras.get("reward", 0)

    if match_rate is not None and 0 < match_rate < 100:
        expected_reward = int(actual_reward / (match_rate / 100))
        loss_sats = expected_reward - actual_reward
    else:
        loss_sats = 0
        match_rate = match_rate if match_rate is not None else 100.0

    # Parallel-friendly price fetch
    price_path = f"/api/v1/historical-price?timestamp={ts}&currency=USD"
    price_data = fetch_worker((price_path, index))
    hist_price = price_data.get('usd', 0) if price_data else 0.0

    return {
        "height": b['height'],
        "match_rate": match_rate,
        "loss_usd": round((loss_sats / 100_000_000) * hist_price, 2),
        "price": hist_price
    }

def fetch_full_ocean_report():
    slug = "ocean"
    all_blocks = []
    last_height = ""
    total_expected = get_pool_stats()

    print(f"--- Parallel OCEAN Audit ---")

    # Stage 1: Fast Header Crawl (Sequential is fine here as it's only ~84 requests)
    with tqdm(total=total_expected, desc="Fetching Headers") as pbar:
        while True:
            path = f"/api/v1/mining/pool/{slug}/blocks/{last_height}"
            batch = fetch_worker((path, 0))
            if not batch: break
            all_blocks.extend(batch)
            last_height = batch[-1]['height']
            pbar.update(len(batch))
            time.sleep(0.1)

    # Stage 2: Parallel Analysis (The slow part)
    processed_data = []
    total_loss_usd = 0.0

    print(f"Analyzing {len(all_blocks)} blocks using {len(MIRRORS)} mirrors...")

    # We use max_workers=10 to keep it respectful but fast
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = {executor.submit(process_block, b, i): b for i, b in enumerate(all_blocks)}

        for future in tqdm(as_completed(futures), total=len(all_blocks), desc="Pricing & Loss"):
            result = future.result()
            processed_data.append(result)
            total_loss_usd += result['loss_usd']

    # Final Output
    processed_data.sort(key=lambda x: x['height'], reverse=True)
    print("-" * 40)
    print(f"TOTAL BLOCKS: {len(processed_data)}")
    print(f"TOTAL LOSS:   ${total_loss_usd:,.2f}")

    with open("ocean_historical_report.json", "w") as f:
        json.dump(processed_data, f, indent=4)

    # Also write pools-3y.json as requested
    pools_3y = fetch_worker(("/api/v1/mining/pools/3y", 0))
    if pools_3y:
        with open("pools-3y.json", "w") as f:
            json.dump(pools_3y, f, indent=4)
        print("Reference file pools-3y.json updated.")

if __name__ == "__main__":
    fetch_full_ocean_report()
