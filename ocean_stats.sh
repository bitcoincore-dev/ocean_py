#!/usr/bin/env bash
# 1. Create the environment
python3 -m venv venv

# 2. Activate it
source venv/bin/activate

# 3. Install requests
pip install --upgrade pip
pip install requests
pip install tqdm
# 4. Run your script
#./ocean_stats.py
#./ocean_health.py
#./ocean_reward_delta.py
#./ocean_total.py
./fetch_pools.py
./ocean_total_loss.py
