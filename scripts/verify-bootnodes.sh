#!/usr/bin/env bash

set -e

if [ $1 = "karura" ]; then
	IMAGE=acala/karura-node:latest
	CHAIN_NAME=karura
	GENESIS=./resources/karura-dist.json
elif [ $1 = "acala" ]; then
	IMAGE=acala/acala-node:latest
	CHAIN_NAME=acala
	GENESIS=./resources/acala-dist.json
else
	echo "not support $1"
	exit 1
fi

docker rm -f bootnotes > /dev/null 2>&1 || true

list=`cat $GENESIS |jq -r .bootNodes[]`

for node in $list;
do
	#echo $node
	docker run \
		   -p 9933:9933 \
		   -d \
		   --rm \
		   --name bootnotes \
		   $IMAGE \
		   --chain=$CHAIN_NAME \
		   --reserved-only \
		   --reserved-nodes=$node \
		   --rpc-port=9933 \
		   --rpc-external \
		   --rpc-cors=all \
		   --rpc-methods=unsafe \
		> /dev/null
	sleep 10s

	echo ""
	echo "Try to connect to $node"
	TRY_TIMES=100
	for ((i = 1; i <= $TRY_TIMES; i++))
	do
		peer=$(curl -sS \
			--connect-timeout 5 -m 5 \
			-H 'Content-Type: application/json' \
			--data '{"id":1,"jsonrpc":"2.0","method":"system_peers"}' \
			localhost:9933 |\
			jq -r '.result')

		peer_num=$(echo $peer | jq 'length')
		peer_id=$(echo $peer | jq -r '.[0].peerId')

		if [ "$peer_num" == "1" ] && [ $peer_id == ${node##*/} ]; then
			echo "Connect to $node succeed."
			break
		fi
		sleep 3s

		if [ $i == $TRY_TIMES ]; then
			FAIL=true
			echo "Connect to $node failed. Node connect to peer id: $peer_id, peer num: $peer_id"
		fi
	done

	docker rm -f bootnotes > /dev/null
done

if [ -n "$FAIL" ]; then
	echo "Bootnotes need to be checked."
	exit 1
fi
