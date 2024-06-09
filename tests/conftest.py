import os
from pathlib import Path

RESOURCE_DIR = Path(__file__).parent / "resources"
os.environ["ENV_PATH"] = str(RESOURCE_DIR / "test.env")
