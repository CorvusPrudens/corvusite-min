aws s3 sync build s3://corvusprudens.com/ \
  --exclude "blog-build" \
  --cache-control "max-age=300"
