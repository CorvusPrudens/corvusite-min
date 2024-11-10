aws cloudformation deploy \
  --region us-east-1 \
  --template-file cloud.yaml \
  --stack-name corvusite \
  --parameter-overrides \
  DomainName=corvusprudens.com
