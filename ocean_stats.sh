#!/usr/bin/env bash
python3 -m venv venv
source venv/bin/activate
pip install --upgrade pip
pip install requests
pip install tqdm
pip install aiohttp
pip install argparse

./ocean_total.py


#./ocean_stats.py
#./ocean_health.py
#./ocean_reward_delta.py
#./ocean_total.py
#./fetch_pools.py
#./ocean_total_loss.py
