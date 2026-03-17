#!/usr/bin/env python3
import requests
import sys
import time
import json
import argparse

def fetch_historical_btc_prices(depth=None):
    # Removed slug and base_blocks_url as they are for ocean mining
    # price_url was for current BTC price, not needed for historical prices by timestamp

    # API endpoints for historical prices
    block_tip_height_url = "https://mempool.space/api/blocks/tip/height"
    block_height_to_hash_url = "https://mempool.space/api/block-height/{}"
    block_hash_to_timestamp_url = "https://mempool.space/api/block/{}"
    historical_price_by_timestamp_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp={}"

    historical_prices = []

    try:
        print(f"--- Starting Historical BTC Price Crawl ---")

        # 1. Get current block height
        current_height_response = requests.get(block_tip_height_url, timeout=10)
        current_height_response.raise_for_status()
        current_height = int(current_height_response.text)

        print(f"Current Block Height: {current_height}")

        start_height = current_height
        if depth:
            end_height = current_height - depth + 1
            if end_height < 0: # Ensure we don't go below block 0
                end_height = 0
        else:
            end_height = 0 # Fetch all history if depth is not specified, up to block 0

        print(f"Fetching prices from block {start_height} down to {end_height}...")

        for height in range(start_height, end_height - 1, -1):
            if (start_height - height + 1) % 10 == 0 or (start_height - height + 1) == 1 or (start_height - height + 1) == depth:
                print(f"Processing block {height} ({start_height - height + 1} of {depth if depth else 'all'})")

            # Step 1: Get block hash from height
            print(f"  Fetching hash for height {height}...")
            hash_response = requests.get(block_height_to_hash_url.format(height), timeout=10)
            hash_response.raise_for_status()
            block_hash = hash_response.text
            print(f"  Fetched hash: {block_hash}")

            # Step 2: Get block timestamp from hash
            print(f"  Fetching timestamp for hash {block_hash}...")
            block_info_response = requests.get(block_hash_to_timestamp_url.format(block_hash), timeout=10)
            block_info_response.raise_for_status()
            block_info = block_info_response.json()
            timestamp = block_info['timestamp']
            print(f"  Fetched timestamp: {timestamp}")

            # Step 3: Get historical price from timestamp
            print(f"  Fetching price for timestamp {timestamp}...")
            price_response = requests.get(historical_price_by_timestamp_url.format(timestamp), timeout=10)
            price_response.raise_for_status()
            btc_usd = price_response.json().get("USD", None)
            print(f"  Fetched price: {btc_usd}")

            if btc_usd is not None:
                historical_prices.append({
                    "height": height,
                    "timestamp": timestamp,
                    "btc_usd": btc_usd
                })

            time.sleep(0.1) # Be kind to the API

        print(f"\nFinal historical_prices list (first 5 items): {historical_prices[:5]}")
        print(f"Total items in historical_prices: {len(historical_prices)}")

        # Save to file
        with open("prices.json", "w") as f:
            json.dump(historical_prices, f, indent=4)
        print(f"Historical prices saved to: prices.json")

    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="Fetch historical Bitcoin (BTC) prices by block height.")
    parser.add_argument(
        "--depth",
        type=int,
        help="Number of historical blocks to fetch prices for. Fetches all available history if not specified."
    )
    args = parser.parse_args()

    fetch_historical_btc_prices(depth=args.depth)

if __name__ == "__main__":
    main()
