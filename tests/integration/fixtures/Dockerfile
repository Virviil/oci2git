FROM alpine:latest

# Add some test files
COPY hello.txt /app/hello.txt
COPY config.json /app/config.json
COPY script.sh /app/script.sh

# Set working directory
WORKDIR /app

# Set environment variables
ENV APP_NAME=test-app
ENV APP_VERSION=1.0.0
ENV DEBUG=true
ENV PATH=/app:$PATH

# Make script executable
RUN chmod +x /app/script.sh

# Set entrypoint
ENTRYPOINT ["/app/script.sh"]
CMD ["--help"]