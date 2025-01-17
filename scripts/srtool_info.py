import sys
import json
from string import Template

tpl = Template("""
```
🏋️ Runtime Size:		$size bytes
🔥 Core Version:		$core_version
🎁 Metadata version:		V$metadata_version
🗳️ system.setCode hash:		$setCode
🗳️ authorizeUpgrade hash:	$authorizeUpgrade
#️⃣ Blake2-256 hash:		$blake
📦 IPFS:			$ipfs
```
""")

data = json.load(open(sys.argv[1], 'r'))

subwasm = data['runtimes']['compressed']['subwasm']
print(tpl.safe_substitute(
    size=subwasm['size'],
    core_version=subwasm['core_version'],
    metadata_version=subwasm['metadata_version'],
    setCode=subwasm['proposal_hash'],
    authorizeUpgrade=subwasm['parachain_authorize_upgrade_hash'],
    ipfs=subwasm['ipfs_hash'],
    blake=subwasm['blake2_256']
))
