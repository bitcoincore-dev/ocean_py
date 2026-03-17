#!/usr/bin/env python3
import requests
import sys
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

if __name__ == "__main__":
    fetch_full_historical_prices()