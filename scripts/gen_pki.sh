#!/bin/bash
set -e

PKI_DIR="$(dirname "$0")/../pki"
mkdir -p "$PKI_DIR"
cd "$PKI_DIR"

echo "=== 生成 CA 密钥和自签名证书 ==="
openssl genrsa -out ca.key 2048
openssl req -x509 -new -nodes -key ca.key -days 365 \
  -subj "/CN=VPN CA" -out ca.crt

echo "=== 生成 Server 密钥和 CSR ==="
openssl genrsa -out server.key 2048
openssl req -new -key server.key \
  -subj "/CN=vpn-server" -out server.csr

echo "=== CA 签发 Server 证书（v3）==="
cat > v3.ext << EOF
basicConstraints=CA:FALSE
keyUsage=digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=DNS:vpn-server
EOF
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key \
  -CAcreateserial -days 365 -out server.crt -extfile v3.ext

echo "=== 验证证书链 ==="
openssl verify -CAfile ca.crt server.crt

echo ""
echo "=== PKI 文件清单 ==="
ls -lh "$PKI_DIR"
echo ""
echo "  部署说明:"
echo "  - ca.crt:  部署到 Host U（客户端校验 Server）"
echo "  - server.crt + server.key: 部署到 VPN Server"
