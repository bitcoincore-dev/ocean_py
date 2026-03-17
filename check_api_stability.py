#!/usr/bin/env python3
import requests
import json
import time

def fetch_blocks_sample():
    url = "https://mempool.space/api/v1/mining/pool/ocean/blocks"
    try:
        response = requests.get(url, timeout=10)
        response.raise_for_status()
        return response.json()[:5] # Fetch first 5 blocks
    except Exception as e:
        print(f"Error fetching blocks: {e}")
        return None

def main():
    print("--- Checking API Stability for Ocean Mining Pool Blocks ---")

    print("Fetching sample 1...")
    sample1 = fetch_blocks_sample()
    if sample1 is None:
        print("Failed to fetch sample 1. Exiting.")
        return

    time.sleep(2) # Wait a bit before fetching again

    print("Fetching sample 2...")
    sample2 = fetch_blocks_sample()
    if sample2 is None:
        print("Failed to fetch sample 2. Exiting.")
        return

    if len(sample1) != len(sample2):
        print("Error: Sample lengths differ. Cannot compare.")
        return

    differences_found = False
    for i in range(len(sample1)):
        block1 = sample1[i]
        block2 = sample2[i]

        height = block1.get('height')
        id_val = block1.get('id')

        # Compare matchRate
        match_rate1 = block1.get('extras', {}).get('matchRate')
        match_rate2 = block2.get('extras', {}).get('matchRate')
        if match_rate1 != match_rate2:
            print(f"Difference in matchRate for Block {height} ({id_val}): Sample 1={match_rate1}, Sample 2={match_rate2}")
            differences_found = True
        
        # Compare reward
        reward1 = block1.get('extras', {}).get('reward')
        reward2 = block2.get('extras', {}).get('reward')
        if reward1 != reward2:
            print(f"Difference in reward for Block {height} ({id_val}): Sample 1={reward1}, Sample 2={reward2}")
            differences_found = True
            
        # Compare expectedFees
        expected_fees1 = block1.get('extras', {}).get('expectedFees')
        expected_fees2 = block2.get('extras', {}).get('expectedFees')
        if expected_fees1 != expected_fees2:
            print(f"Difference in expectedFees for Block {height} ({id_val}): Sample 1={expected_fees1}, Sample 2={expected_fees2}")
            differences_found = True

    if not differences_found:
        print("\nNo differences found in matchRate, reward, or expectedFees for the sampled blocks.")
    else:
        print("\nDifferences found in sampled blocks. API data may not be perfectly static for these fields.")

if __name__ == "__main__":
    main()
