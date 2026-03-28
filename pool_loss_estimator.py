#!/usr/bin/env python3
import argparse
import requests
import sys
import time
import json

def fetch_mempool_pools(time_period='1y', limit=None):
    api_url = f"https://mempool.space/api/v1/mining/pools/{time_period}"
    print(f"--- Fetching active pools from {api_url} ---")
    try:
        response = requests.get(api_url, timeout=15)
        response.raise_for_status()
        pools_data = response.json()
        print(f"DEBUG: Type of pools_data: {type(pools_data)}")
        print(f"DEBUG: Content of pools_data (first 200 chars): {str(pools_data)[:200]}")

        if not isinstance(pools_data, dict) or 'pools' not in pools_data:
            print("Error: Expected a dictionary with a 'pools' key from mempool.space API, but received a different structure.")
            return []
        
        # Now, pools_data should be the list of pools inside the 'pools' key
        pools_list = pools_data['pools']
        if not isinstance(pools_list, list):
            print("Error: Expected a list under the 'pools' key, but received a different type.")
            return []

        pool_slugs = [pool.get('slug') for pool in pools_list if pool.get('slug')]
        if limit and len(pool_slugs) > limit:
            print(f"Limiting fetched pools to {limit}.")
            pool_slugs = pool_slugs[:limit]
        print(f"Fetched {len(pool_slugs)} pools from mempool.space.")
        return pool_slugs
    except requests.exceptions.RequestException as e:
        print(f"Error fetching pools from mempool.space: {e}")
        return []

def fetch_full_historical_prices():
    api_url = "https://mempool.space/api/v1/historical-price?currency=USD&timestamp=0"
    output_file = "prices.json"

    print(f"--- Starting Full Historical BTC Price Fetch from {api_url} ---")

    try:
        response = requests.get(api_url, timeout=30)
        response.raise_for_status()  # Raise an exception for HTTP errors (4xx or 5xx)

        historical_data = response.json()

        if not historical_data:
            print("No historical price data received.")
            sys.exit(1)

        with open(output_file, "w") as f:
            json.dump(historical_data, f, indent=4)
        print(f"Full historical prices saved to: {output_file}")

    except requests.exceptions.Timeout:
        print("Error: The request timed out.")
        sys.exit(1)
    except requests.exceptions.RequestException as e:
        print(f"Error fetching data: {e}")
        sys.exit(1)
    except json.JSONDecodeError:
        print("Error: Could not decode JSON response from the API.")
        sys.exit(1)
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        sys.exit(1)

def analyze_pool_loss(pool_slug, depth=None):
    base_blocks_url = f"https://mempool.space/api/v1/mining/pool/{pool_slug}/blocks"

    all_blocks = []
    last_height = None

    try:
        # Load historical prices from prices.json
        try:
            with open("prices.json", "r") as f:
                historical_data = json.load(f)
            price_lookup = {item['time']: item['USD'] for item in historical_data.get('prices', [])}
            sorted_timestamps = sorted(price_lookup.keys())
            print(f"Loaded {len(price_lookup)} historical prices from prices.json for {pool_slug}")
        except FileNotFoundError:
            print(f"prices.json not found for {pool_slug}. Attempting to fetch full historical prices...")
            fetch_full_historical_prices() # Call the new function to fetch prices
            # After fetching, try loading again
            with open("prices.json", "r") as f:
                historical_data = json.load(f)
            price_lookup = {item['time']: item['USD'] for item in historical_data.get('prices', [])}
            sorted_timestamps = sorted(price_lookup.keys())
            print(f"Loaded {len(price_lookup)} historical prices from prices.json (after fetch) for {pool_slug}")

        print(f"--- Starting Full History Crawl for {pool_slug.upper()} ---")

        while True:
            url = f"{base_blocks_url}/{last_height}" if last_height else base_blocks_url

            response = requests.get(url, timeout=10)
            response.raise_for_status()
            batch = response.json()

            if not batch:
                break

            all_blocks.extend(batch)
            last_height = batch[-1]['height']

            print(f"Fetched {len(all_blocks)} blocks... (Current Height: {last_height})")

            if depth and len(all_blocks) >= depth:
                all_blocks = all_blocks[:depth] # Trim to exact depth if overshot
                break

            time.sleep(0.5)

        total_loss_usd = 0 # This will be calculated in the calling function
        processed_data = []

        # Removed direct print of header and dashes

        for b in all_blocks:
            # print(f"DEBUG: Processing block: {b}") # Uncomment for verbose block debugging
            extras = b.get("extras", {})
            
            raw_match_rate = extras.get("matchRate")
            if raw_match_rate is None:
                match_rate = 0
            else:
                match_rate = round(raw_match_rate, 2)

            raw_actual_reward = extras.get("reward")
            if raw_actual_reward is None:
                actual_reward = 0
            else:
                actual_reward = raw_actual_reward

            expected_reward = actual_reward # Default if match_rate is 0 or 100
            if match_rate > 0 and match_rate < 100:
                expected_reward = int(round((actual_reward * 100) / match_rate))
            loss_sats = expected_reward - actual_reward

            timestamp = b.get('timestamp')
            # print(f"DEBUG: Raw timestamp for block {b.get('height')}: {timestamp}") # Debug print for raw timestamp
            btc_usd = 0
            if timestamp:
                closest_timestamp = None
                for hist_ts in sorted_timestamps:
                    if hist_ts <= timestamp:
                        closest_timestamp = hist_ts
                    else:
                        break
                if closest_timestamp is not None:
                    btc_usd = price_lookup.get(closest_timestamp, 0)

            loss_usd = (loss_sats / 100_000_000) * btc_usd

            processed_data.append({
                "height": b['height'],
                "health": match_rate,
                "actual_reward": actual_reward, # Add actual_reward for context
                "expected_reward": expected_reward, # Add expected_reward for loss quotient
                "loss_sats": loss_sats,
                "loss_usd": round(loss_usd, 2),
                "btc_usd": btc_usd,
                "timestamp": timestamp # Add timestamp to processed_data
            })

        return processed_data

    except Exception as e:
        print(f"Error analyzing pool {pool_slug}: {e}")
        return None # Return None to indicate failure, let main handle sys.exit

def compare_pool_losses(ocean_slug, other_pool_slugs, depth):
    print(f"\n--- Comparing Losses: {ocean_slug.upper()} vs. {', '.join(p.upper() for p in other_pool_slugs)} ---")

    # 1. Analyze Ocean's actual losses
    print(f"\nAnalyzing {ocean_slug.upper()}...")
    ocean_processed_data = analyze_pool_loss(ocean_slug, depth)
    if ocean_processed_data is None:
        print(f"Failed to retrieve data for {ocean_slug.upper()}. Exiting.")
        sys.exit(1)

    # Determine the actual number of blocks fetched for Ocean
    actual_ocean_depth = len(ocean_processed_data)
    print(f"Ocean analyzed {actual_ocean_depth} blocks.")

    # Pre-sort Ocean data by timestamp for efficient searching later
    ocean_data_by_timestamp = sorted(ocean_processed_data, key=lambda x: x.get('timestamp', 0))

    # Calculate Ocean's total loss for reference
    ocean_total_loss_usd = sum(item['loss_usd'] for item in ocean_processed_data)
    print(f"TOTAL CUMULATIVE LOSS for {ocean_slug.upper()}: ${ocean_total_loss_usd:,.2f}")

    for other_pool_slug in other_pool_slugs:
        print(f"\nAnalyzing {other_pool_slug.upper()} (limited to {actual_ocean_depth} blocks)...")
        # Use the actual_ocean_depth for other pools
        other_pool_processed_data = analyze_pool_loss(other_pool_slug, actual_ocean_depth)
        if other_pool_processed_data is None:
            print(f"Failed to retrieve data for {other_pool_slug.upper()}. Skipping.")
            continue

        other_pool_data_by_timestamp = sorted(other_pool_processed_data, key=lambda x: x.get('timestamp', 0))
        other_pool_blocks_used = [False] * len(other_pool_data_by_timestamp) # Track used blocks

        estimated_other_pool_loss_usd = 0
        comparisons_made = 0
        TIME_DIFFERENCE_THRESHOLD = 3600 # 1 hour in seconds, adjust as needed

        print(f"\nEstimating loss for {other_pool_slug.upper()} based on {ocean_slug.upper()} rules...")
        print(f"\n{'Ocean Height':<12} | {'Ocean TS':<10} | {'Ocean Loss($)':<14} | {'Ocean Actual ($)':<16} | {'Other Height':<12} | {'Other TS':<10} | {'Other Pool Actual ($)':<22} | {'Est. Loss($)':<14}")
        print("-" * 90)

        for ocean_block in ocean_processed_data:
            if ocean_block['loss_sats'] > 0 and ocean_block['expected_reward'] > 0:
                ocean_loss_quotient = ocean_block['loss_sats'] / ocean_block['expected_reward']
                ocean_block_timestamp = ocean_block.get('timestamp', 0)

                # Find the closest UNUSED block in other_pool_data by timestamp
                closest_other_block = None
                closest_other_block_index = -1
                min_time_diff = float('inf')

                for other_block_idx, other_block in enumerate(other_pool_data_by_timestamp):
                    if other_pool_blocks_used[other_block_idx]:
                        continue # Skip if this block has already been used

                    other_block_timestamp = other_block.get('timestamp', 0)
                    time_diff = abs(ocean_block_timestamp - other_block_timestamp)

                    if time_diff < min_time_diff:
                        min_time_diff = time_diff
                        closest_other_block = other_block
                        closest_other_block_index = other_block_idx
                    # Optimization: if current other_block_timestamp is already much larger than
                    # ocean_block_timestamp, and we are iterating in sorted order, we can break.
                    # Re-enabling the optimization with a check to ensure at least one block was found
                    if other_block_timestamp > ocean_block_timestamp + TIME_DIFFERENCE_THRESHOLD and min_time_diff != float('inf'):
                         break

                if closest_other_block and closest_other_block_index != -1 and min_time_diff <= TIME_DIFFERENCE_THRESHOLD:
                    # Mark the closest block as used
                    other_pool_blocks_used[closest_other_block_index] = True
                    # Estimate loss for the other pool's block
                    other_pool_estimated_loss_sats = ocean_loss_quotient * closest_other_block['actual_reward']
                    other_pool_estimated_loss_usd = (other_pool_estimated_loss_sats / 100_000_000) * closest_other_block['btc_usd']
                    estimated_other_pool_loss_usd += other_pool_estimated_loss_usd
                    comparisons_made += 1
                    ocean_actual_usd = (ocean_block['actual_reward'] / 100_000_000) * ocean_block['btc_usd']
                    other_pool_actual_usd = (closest_other_block['actual_reward'] / 100_000_000) * closest_other_block['btc_usd']
                    
                    print(f"{ocean_block['height']:<12} | {ocean_block_timestamp:<10} | {ocean_block['loss_usd']:<14.2f} | {ocean_actual_usd:<16.2f} | {closest_other_block['height']:<12} | {closest_other_block.get('timestamp', 0):<10} | {other_pool_actual_usd:<22.2f} | {other_pool_estimated_loss_usd:<14.2f}")

        print("-" * 90)
        print(f"TOTAL ESTIMATED CUMULATIVE LOSS for {other_pool_slug.upper()} (based on {ocean_slug.upper()} rules): ${estimated_other_pool_loss_usd:,.2f} ({comparisons_made} blocks compared)")

    # No return value needed, just prints results

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Estimate cumulative loss for mining pools using Ocean's rules.")
    parser.add_argument(
        "--ocean-slug",
        type=str,
        default="ocean",
        help="The slug of the reference mining pool (default: ocean)"
    )
    parser.add_argument(
        "--other-pools",
        type=str,
        help="Comma-separated list of other mining pool slugs to analyze (e.g., antpool,f2pool)"
    )
    parser.add_argument(
        "--depth",
        type=int,
        help="Number of historical blocks to fetch and analyze for each pool. Analyzes all if not specified."
    )
    args = parser.parse_args()

    if args.other_pools:
        other_pool_slugs = [slug.strip() for slug in args.other_pools.split(',') if slug.strip()]
    else:
        print("No --other-pools specified. Attempting to fetch active pools from mempool.space...")
        other_pool_slugs = fetch_mempool_pools(limit=args.depth)
        if not other_pool_slugs:
            print("Error: Could not fetch any pools from mempool.space. Please specify --other-pools manually.")
            sys.exit(1)
        else:
            print(f"Using fetched pools: {', '.join(other_pool_slugs)}")

    compare_pool_losses(ocean_slug=args.ocean_slug, other_pool_slugs=other_pool_slugs, depth=args.depth)
