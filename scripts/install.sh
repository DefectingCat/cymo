#!/usr/bin/env bash

OS=""
case "$OSTYPE" in
    solaris*) echo "SOLARIS" ;;
    darwin*)  OS="apple-darwin" ;;
    linux*)   echo "LINUX" ;;
    bsd*)     echo "BSD" ;;
    msys*)    echo "WINDOWS" ;;
    cygwin*)  echo "ALSO WINDOWS" ;;
    *)        echo "unknown: $OSTYPE" ;;
esac

arch=""
case "$(uname -m)" in
    arm64*) arch="aarch64-" ;;
    *) echo "unknown arch: $(uname -m)" ;;
esac

echo "$arch$OS"

function detectCurl() {
    if !(command -v curl >/dev/null 2>&1); then
        echo "curl does not exist"
        exit 1
    fi
}

detectCurl

url="http://192.168.1.22/lilinjun/cymo/uploads/2a759444eeeeb1e7e26f6326aaa6db5b/cymo-$arch$OS"
curl "$url" -o "cymo"
