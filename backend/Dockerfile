FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl gnupg ffmpeg libchromaprint-tools python3 python3-pip \
        tesseract-ocr tesseract-ocr-eng tesseract-ocr-pol mkvtoolnix calibre \
    && install -m 0755 -d /etc/apt/keyrings \
    && curl -fsSL https://download.docker.com/linux/debian/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg \
    && chmod a+r /etc/apt/keyrings/docker.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/debian bookworm stable" > /etc/apt/sources.list.d/docker.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends docker-ce-cli docker-compose-plugin \
    && apt-get purge -y gnupg \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

RUN echo 'setuptools<81' > /tmp/pip-constraints.txt \
    && pip3 install --break-system-packages -c /tmp/pip-constraints.txt ffsubsync srt pgsrip faster-whisper \
    && rm /tmp/pip-constraints.txt \
    || echo "[runtime] python deps install skipped"

RUN curl -fsSL https://github.com/kaegi/alass/releases/download/v2.0.0/alass-linux64 -o /usr/local/bin/alass \
    && chmod +x /usr/local/bin/alass

RUN ebook-convert --version

COPY scripts/pw-whisper-sync.py /usr/local/bin/pw-whisper-sync
RUN chmod +x /usr/local/bin/pw-whisper-sync

WORKDIR /opt/pleasewatch
ENV STATIC_DIR=/opt/pleasewatch/static
ENV HOST=0.0.0.0
ENV PORT=3000
EXPOSE 3000
CMD ["pleasewatch"]
