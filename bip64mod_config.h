/* BIP-64MOD + GCC Integration Header */
#define BIP64_MOD_ENABLED 1
#define OCEAN_TIDES_SUPPORT 1
#define MAX_METADATA_PEERS 128

typedef struct {
    char peer_addr[64];
    uint32_t version_mod;
    uint64_t session_id;
} BIP64ModContext;
