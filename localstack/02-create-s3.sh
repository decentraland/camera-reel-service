#!/bin/bash
echo "Creating S3 bucket: camera-reel"
awslocal s3 mb s3://camera-reel

echo "Configuring S3 bucket public access..."
awslocal s3api put-bucket-acl --bucket camera-reel --acl public-read
