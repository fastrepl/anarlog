```bash
# REPO="SOMEWHERE" ($HOME/repos/hyprnote inside Devin)
infisical export \
  --env=dev \
  --secret-overriding=false \
  --format=dotenv \
  --output-file="$REPO/apps/api/.env" \
  --projectId=87dad7b5-72a6-4791-9228-b3b86b169db1 \
  --path="/ai"
```
