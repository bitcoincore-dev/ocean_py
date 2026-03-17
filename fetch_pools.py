#!/usr/bin/env python3
import requests
import json
import sys

def save_pool_data():
    # Primary URL and Failover
    failover_url = "https://mempool.sweetsats.io/api/v1/mining/pools/1y"
    url = "https://mempool.space/api/v1/mining/pools/1y"

    output_file = "pools-1y.json"

    print(f"--- Fetching Pool Data (3Y) ---")
    try:
        # Attempt to fetch from sweetsats
        response = requests.get(url, timeout=15)

        # Fallback logic
        if response.status_code != 200:
            print(f"[-] SweetSats returned {response.status_code}. Trying mempool.space...")
            response = requests.get(failover_url, timeout=15)

        response.raise_for_status()
        data = response.json()

        # Write to JSON file
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=4)

        print(f"[+] Successfully wrote {len(data)} pool entries to {output_file}")

    except Exception as e:
        print(f"[!] Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    save_pool_data()
