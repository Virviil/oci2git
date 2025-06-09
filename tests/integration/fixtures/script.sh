#!/bin/sh

echo "Starting $APP_NAME v$APP_VERSION"
echo "Debug mode: $DEBUG"
echo "Working directory: $(pwd)"
echo "Available files:"
ls -la /app/

case "$1" in
  "--help")
    echo "Usage: $0 [--help|--config|--hello]"
    echo "  --help    Show this help message"
    echo "  --config  Display configuration"
    echo "  --hello   Display hello message"
    ;;
  "--config")
    echo "Configuration:"
    cat /app/config.json
    ;;
  "--hello")
    echo "Hello message:"
    cat /app/hello.txt
    ;;
  *)
    echo "Unknown option: $1"
    echo "Use --help for usage information"
    exit 1
    ;;
esac