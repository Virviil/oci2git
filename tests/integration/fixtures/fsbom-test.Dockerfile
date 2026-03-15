FROM busybox:1.37

# Layer 1: Create initial files
RUN mkdir -p /app && \
    echo "hello world" > /app/hello.txt && \
    echo "static content" > /app/static.txt

# Layer 2: Add subdirectory, symlinks, and a script
RUN mkdir -p /app/sub && \
    echo "sub content" > /app/sub/data.txt && \
    echo '#!/bin/sh\necho hello' > /app/run.sh && \
    chmod +x /app/run.sh && \
    ln -s /app/hello.txt /app/hello-link.txt && \
    ln -s ../run.sh /app/sub/run-link.sh

# Layer 3: Modify hello.txt, add new file, delete static.txt
RUN echo "hello updated" > /app/hello.txt && \
    echo "new file" > /app/new.txt && \
    rm /app/static.txt
