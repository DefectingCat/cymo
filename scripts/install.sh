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
