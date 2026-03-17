#!/usr/bin/env bash
python3 -m venv venv
source venv/bin/activate
pip install --upgrade pip
pip install requests
pip install tqdm
pip install aiohttp
pip install argparse

./ocean_total.py
