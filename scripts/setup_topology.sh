#!/bin/bash
set -e

echo "=== 清理旧环境 ==="
for ns in u s v; do
    sudo ip netns del $ns 2>/dev/null || true
done

echo "=== 创建三个网络命名空间 ==="
sudo ip netns add u   # Host U (VPN客户端)
sudo ip netns add s   # VPN Server
sudo ip netns add v   # Host V (内网主机)

echo "=== 创建 veth pair ==="
sudo ip link add u-s type veth peer name s-u
sudo ip link add s-v type veth peer name v-s

echo "=== 将 veth 端点分配到 netns ==="
sudo ip link set u-s netns u
sudo ip link set s-u netns s
sudo ip link set s-v netns s
sudo ip link set v-s netns v

echo "=== 分配 IP 地址 ==="
sudo ip netns exec u ip addr add 192.168.10.2/24 dev u-s
sudo ip netns exec u ip link set u-s up
sudo ip netns exec u ip link set lo up

sudo ip netns exec s ip addr add 192.168.10.1/24 dev s-u
sudo ip netns exec s ip link set s-u up
sudo ip netns exec s ip addr add 192.168.20.1/24 dev s-v
sudo ip netns exec s ip link set s-v up
sudo ip netns exec s ip link set lo up

sudo ip netns exec v ip addr add 192.168.20.2/24 dev v-s
sudo ip netns exec v ip link set v-s up
sudo ip netns exec v ip link set lo up

echo "=== 配置 Server 转发 ==="
sudo ip netns exec s sysctl -w net.ipv4.ip_forward=1 > /dev/null

echo "=== Host V 添加默认路由 ==="
sudo ip netns exec v ip route add default via 192.168.20.1

# 注意：不要添加 iptables DROP 规则来隔离 U 与 V。
# 路由表本身已确保 192.168.20.0/24 指向 tun0（VPN 接口），
# 不会走物理网卡 u-s，因此天然隔离。
# 如果加 DROP 规则，反而会拦截经 VPN 隧道转发的合法流量。

echo ""
echo "=== 连通性测试 ==="
echo "--- U -> S ---"
sudo ip netns exec u ping -c 1 -W 1 192.168.10.1
echo "--- S -> V ---"
sudo ip netns exec s ping -c 1 -W 1 192.168.20.2
echo "--- U -> V (应不通) ---"
sudo ip netns exec u ping -c 1 -W 1 192.168.20.2 || echo "  ✓ 预期不通，隔离生效"

echo ""
echo "=== 拓扑搭建完成 ==="
echo "  Host U:  192.168.10.2  (netns: u)"
echo "  Server:  192.168.10.1 / 192.168.20.1  (netns: s)"
echo "  Host V:  192.168.20.2  (netns: v)"
