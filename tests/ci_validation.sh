#!/bin/bash
set -e

# Setup Paths
WORKSPACE_ROOT=$(pwd)
SBC_RS_PATH="$WORKSPACE_ROOT/sbc-rs"
TEMPLATE_PATH="$WORKSPACE_ROOT/../sing-box-config-templates/config.template.json"
OUTPUT_PATH="$WORKSPACE_ROOT/tests/config.gen.json"

echo "=== CI Validation Started ==="
echo "Building sbc-rs..."
cd "$SBC_RS_PATH"
cargo build --quiet
SBC_BIN="$SBC_RS_PATH/target/debug/sbc-rs"
cd "$WORKSPACE_ROOT"

if [ ! -f "$TEMPLATE_PATH" ]; then
    echo "Error: Template not found at $TEMPLATE_PATH"
    exit 1
fi

echo "Generating Mock Environment..."
# Export Mock Variables
export CLASH_API_SECRET="test_secret"
export MIXED_PROXY_USERNAME="admin"
export MIXED_PROXY_PASSWORD="123"
export PROVIDER_NAME_1="Provider1"
export PROVIDER_NAME_2="Provider2"
export PROVIDER_NAME_3="Provider3"
export SUB_URL_1="http://example.com/1"
export SUB_URL_2="http://example.com/2"
export SUB_URL_3="http://example.com/3"

# JSON Injection Mocks (Empty lists/objects for simplicity, or specific test values)
export DNS_SERVERS='[{"tag":"injected_dns","address":"1.1.1.1","detour":"DIRECT"}]'
export DNS_RULES_MID='{"rule_set":"geosite-category-ads-all","server":"local","action":"reject"}'
export DNS_RULES_BOTTOM='{"server":"local"}'
export ROUTE_RULES_TOP='{"protocol":"dns","action":"hijack-dns"}'
export ROUTE_RULES_MID='{"ip_cidr":["1.0.0.1/32"],"outbound":"DIRECT"}'
export ROUTE_RULES_BOTTOM='{"port":80,"outbound":"DIRECT"}'
export ROUTE_RULE_SETS='{"tag":"test-rule-set","type":"local","format":"source","path":"/tmp/test.json"}'
export INBOUNDS_TOP='{"type":"mixed","tag":"mixed-in","listen":"::","listen_port":2080}'
export INBOUNDS_BOTTOM='{"type":"direct","tag":"dns-in-2","network":"udp"}'
export EXPERIMENTAL_CLASH_API="" # Should be ignored/empty
export EXPERIMENTAL_CACHE_FILE="" # Should be ignored/empty

# Create dummy rule-set file (valid source format)
echo '{"version": 1, "rules": []}' > /tmp/test.json

echo "Running sbc-rs..."
"$SBC_BIN" --template "$TEMPLATE_PATH" --output "$OUTPUT_PATH"

echo "Validating Output with sing-box check..."
if command -v sing-box &> /dev/null; then
    # sing-box check needs the rulesets referenced in config to exist? 
    # Usually check -c verifies syntax. If it needs ext resources, it might fail?
    # verify syntax only?
    # sing-box check loads config.
    sing-box check -c "$OUTPUT_PATH" || { echo "sing-box check failed!"; exit 1; }
    echo "sing-box check PASSED."
else
    echo "Warning: sing-box binary not found, skipping syntax check. (JSON structure is valid if sbc-rs succeeded)"
fi

echo "=== CI Validation PASSED ==="
rm -f "$OUTPUT_PATH"
