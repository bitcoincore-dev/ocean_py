#!/usr/bin/env python3
import requests
import json
import os

def fetch_ocean_data():
    """
    Fetches and displays mining data for the OCEAN pool using the mempool.space API.
    """
    # API Endpoints for the OCEAN pool
    base_url = "https://mempool.space/api/v1/mining"
    slug = "ocean"

    endpoints = {
        "Pool Details": f"{base_url}/pool/{slug}",
        "Hashrate History": f"{base_url}/pool/{slug}/hashrate",
        "Recent Blocks": f"{base_url}/pool/{slug}/blocks"
    }

    print(f"--- Querying mempool.space for Pool: {slug.upper()} ---")

    for title, url in endpoints.items():
        try:
            response = requests.get(url)
            response.raise_for_status()
            data = response.json()

            print(f"\n[+] {title}:")
            # Print a formatted preview of the data
            if isinstance(data, list):
                # Print first 2 items if it's a list (e.g., blocks or hashrate history)
                print(json.dumps(data[:2], indent=4))
                print(f"... ({len(data)} total items returned)")
            else:
                print(json.dumps(data, indent=4))

        except requests.exceptions.RequestException as e:
            print(f"[-] Error fetching {title}: {e}")

def generate_ocean_config_env():
    """
    Sets local environment variables for the BIP-64MOD + GCC context
    using the official pool address found in the documentation.
    """
    os.environ["POOL_URL"] = "mine.ocean.xyz:3334"
    os.environ["POOL_API_SLUG"] = "ocean"

    print("\n--- Local Environment Configured ---")
    print(f"POOL_URL: {os.environ.get('POOL_URL')}")
    print(f"API_SLUG: {os.environ.get('POOL_API_SLUG')}")

if __name__ == "__main__":
    # Ensure dependencies are available
    try:
        import requests
    except ImportError:
        print("Error: 'requests' library not found. Install it with: pip install requests")
        exit(1)

    # 1. Configure the local environment
    generate_ocean_config_env()

    # 2. Fetch live data from the API
    fetch_ocean_data()
