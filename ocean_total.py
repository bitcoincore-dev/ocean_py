#!/usr/bin/env python3
import argparse
import requests
import sys
import time
import json

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

def fetch_all_ocean_blocks(depth=1000):
    slug = "ocean"
    base_blocks_url = f"https://mempool.space/api/v1/mining/pool/{slug}/blocks"
    # price_url and direct BTC price fetch removed, using prices.json for historical data

    all_blocks = []
    last_height = None

    try:
        # Load historical prices from prices.json
        try:
            with open("prices.json", "r") as f:
                historical_data = json.load(f)

            # The historical data is under a 'prices' key, and each item has 'time' and 'USD'
            price_lookup = {item['time']: item['USD'] for item in historical_data.get('prices', [])}
            sorted_timestamps = sorted(price_lookup.keys())
            print(f"Loaded {len(price_lookup)} historical prices from prices.json")
        except FileNotFoundError:
            print("prices.json not found. Attempting to fetch full historical prices...")
            fetch_full_historical_prices() # Call the new function to fetch prices
            # After fetching, try loading again
            with open("prices.json", "r") as f:
                historical_data = json.load(f)
            price_lookup = {item['time']: item['USD'] for item in historical_data.get('prices', [])}
            sorted_timestamps = sorted(price_lookup.keys())
            print(f"Loaded {len(price_lookup)} historical prices from prices.json (after fetch)")

        print(f"--- Starting Full History Crawl for OCEAN ---")

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
            match_rate = round(extras.get("matchRate", 0), 2) # Explicitly round match_rate
            actual_reward = extras.get("reward", 0)

            if match_rate > 0 and match_rate < 100:
                # Calculate expected_reward with more robust precision
                # Convert to integer after multiplication and division to maintain precision
                expected_reward = int(round((actual_reward * 100) / match_rate))
                loss_sats = expected_reward - actual_reward
            else:
                loss_sats = 0

            # Get BTC price for this block from the lookup using its timestamp
            timestamp = b.get('timestamp')
            btc_usd = 0
            if timestamp:
                # Find the closest timestamp in historical_prices_data that is <= block_timestamp
                closest_timestamp = None
                for hist_ts in sorted_timestamps:
                    if hist_ts <= timestamp:
                        closest_timestamp = hist_ts
                    else:
                        break # Timestamps are sorted, so we've passed the block's timestamp
                if closest_timestamp is not None:
                    btc_usd = price_lookup.get(closest_timestamp, 0)

            # Debug print to verify retrieved BTC price
            print(f"Block Height: {b.get('height')}, Block Timestamp: {timestamp}, Closest Price Timestamp: {closest_timestamp}, BTC USD: {btc_usd}")

            loss_usd = (loss_sats / 100_000_000) * btc_usd
            total_loss_usd += loss_usd

            processed_data.append({
                "height": b['height'],
                "health": match_rate,
                "loss_sats": loss_sats,
                "loss_usd": round(loss_usd, 2),
                "btc_usd": btc_usd # Add btc_usd to the output
            })

            # Print a sample of the first few
            if len(processed_data) <= depth:
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
    parser = argparse.ArgumentParser(description="Fetch and analyze OCEAN mining pool data.")
    parser.add_argument("--depth", type=int, default=1000, help="Number of sample blocks to print.")
    args = parser.parse_args()

    fetch_all_ocean_blocks(depth=args.depth)
