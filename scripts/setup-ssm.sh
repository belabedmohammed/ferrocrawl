#!/bin/bash
# Setup SSM parameters for Ferrocrawl
# Usage: ./scripts/setup-ssm.sh [stage]
# Example: ./scripts/setup-ssm.sh prod

set -e

STAGE="${1:-prod}"
REGION="us-east-1"

echo "Setting up SSM parameters for stage: $STAGE (region: $REGION)"
echo ""

# Anthropic API key — shared with Davoxi services
# This is the same key used by ai-phone-agent-rust
EXISTING_KEY=$(aws ssm get-parameter \
  --name "/ai-phone-agent-rust/${STAGE}/anthropic-api-key" \
  --with-decryption \
  --query "Parameter.Value" \
  --output text \
  --region "$REGION" 2>/dev/null || echo "")

if [ -n "$EXISTING_KEY" ] && [ "$EXISTING_KEY" != "None" ]; then
  echo "Anthropic API key already exists at /ai-phone-agent-rust/${STAGE}/anthropic-api-key"
  echo "  Ferrocrawl will reuse this shared key (no action needed)"
else
  echo "WARNING: No Anthropic key found at /ai-phone-agent-rust/${STAGE}/anthropic-api-key"
  echo "  The /v1/extract endpoint will not work without it."
  echo "  To set it:"
  echo "    aws ssm put-parameter \\"
  echo "      --name \"/ai-phone-agent-rust/${STAGE}/anthropic-api-key\" \\"
  echo "      --value \"sk-ant-...\" \\"
  echo "      --type SecureString \\"
  echo "      --region ${REGION}"
fi

echo ""

# Ferrocrawl API keys (for auth)
echo "Setting Ferrocrawl API keys..."
read -rp "Enter comma-separated API keys (or press Enter to skip): " API_KEYS

if [ -n "$API_KEYS" ]; then
  aws ssm put-parameter \
    --name "/ferrocrawl/${STAGE}/api-keys" \
    --value "$API_KEYS" \
    --type SecureString \
    --overwrite \
    --region "$REGION"
  echo "  Set /ferrocrawl/${STAGE}/api-keys"
else
  echo "  Skipped (no auth — all endpoints public)"
fi

echo ""

# Anthropic model
aws ssm put-parameter \
  --name "/ferrocrawl/${STAGE}/anthropic-model" \
  --value "claude-sonnet-4-20250514" \
  --type String \
  --overwrite \
  --region "$REGION"
echo "  Set /ferrocrawl/${STAGE}/anthropic-model = claude-sonnet-4-20250514"

echo ""
echo "Done. SSM parameters for stage '${STAGE}':"
echo ""
echo "  /ai-phone-agent-rust/${STAGE}/anthropic-api-key  (shared, SecureString)"
echo "  /ferrocrawl/${STAGE}/api-keys                    (SecureString)"
echo "  /ferrocrawl/${STAGE}/anthropic-model              (String)"
echo ""
echo "Deploy with: serverless deploy --stage ${STAGE}"
