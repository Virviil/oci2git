FROM alpine:3.18

# Create directory structure for comprehensive link testing
RUN mkdir -p /app/bin /app/lib /app/data /app/target_dir /app/links

# ==== Test 1: Regular files with hardlinks ====
RUN echo "#!/bin/sh" > /app/bin/original.sh && \
    echo "echo 'Hello from original'" >> /app/bin/original.sh && \
    chmod +x /app/bin/original.sh

# Create hardlinks to the script
RUN ln /app/bin/original.sh /app/bin/hardlink1.sh && \
    ln /app/bin/original.sh /app/bin/hardlink2.sh

# ==== Test 2: Library files with hardlinks (common in containers) ====
RUN echo "Library content v1" > /app/lib/libtest.so.1.0

# Hardlink to the library with different versions
RUN ln /app/lib/libtest.so.1.0 /app/lib/libtest.so.1 && \
    ln /app/lib/libtest.so.1.0 /app/lib/libtest.so

# ==== Test 3: Regular symlinks (relative paths) ====
RUN ln -s original.sh /app/bin/relative-symlink.sh

# ==== Test 4: Absolute symlinks ====
RUN ln -s /app/bin/original.sh /app/bin/absolute-symlink.sh

# ==== Test 5: Empty files with hardlinks and symlinks ====
RUN touch /app/data/empty.txt && \
    ln /app/data/empty.txt /app/data/empty-hardlink.txt && \
    ln -s /app/data/empty.txt /app/data/empty-symlink.txt

# ==== Test 6: Symlink pointing to another symlink (chain) ====
RUN ln -s /app/bin/absolute-symlink.sh /app/bin/symlink-chain.sh

# ==== Test 7: Directory with content, then symlink to that directory ====
RUN mkdir -p /app/target_dir && \
    echo "File in target directory" > /app/target_dir/file.txt && \
    ln -s /app/target_dir /app/links/dir-symlink

# ==== Test 8: Symlink to a directory with absolute path ====
RUN ln -s /app/target_dir /app/links/dir-symlink-absolute

# ==== Test 9: Broken symlink (target doesn't exist) ====
RUN ln -s /app/nonexistent/file.txt /app/links/broken-symlink.txt

# ==== Test 10: Multiple hardlinks to same empty file ====
RUN touch /app/data/empty2.txt && \
    ln /app/data/empty2.txt /app/data/empty2-link1.txt && \
    ln /app/data/empty2.txt /app/data/empty2-link2.txt && \
    ln /app/data/empty2.txt /app/data/empty2-link3.txt

# ==== Test 11: File with content, multiple links ====
RUN echo "Data file 1" > /app/data/file1.txt && \
    ln /app/data/file1.txt /app/data/file1-hardlink.txt && \
    ln -s /app/data/file1.txt /app/data/file1-symlink.txt

# ==== Test 12: Relative symlink pointing up directories ====
RUN ln -s ../../bin/original.sh /app/data/relative-up-symlink.sh

# Verify all links were created correctly
RUN echo "=== Hardlinks in /app/bin ===" && ls -li /app/bin/ && \
    echo "=== Hardlinks in /app/lib ===" && ls -li /app/lib/ && \
    echo "=== Links in /app/data ===" && ls -li /app/data/ && \
    echo "=== Directory links ===" && ls -li /app/links/ && \
    echo "=== Target directory ===" && ls -li /app/target_dir/

CMD ["/app/bin/original.sh"]
