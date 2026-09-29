#!/usr/bin/env python3
"""Seed GreenMail with sample messages for mymail GUI / manual testing."""

from __future__ import annotations

import argparse
import smtplib
import sys
import time
from email.message import EmailMessage
from email.utils import formatdate, make_msgid


def wait_smtp(host: str, port: int, timeout: float = 20.0) -> None:
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            with smtplib.SMTP(host, port, timeout=2) as smtp:
                smtp.noop()
            return
        except Exception as exc:  # noqa: BLE001 - retry until timeout
            last_error = exc
            time.sleep(0.4)
    raise SystemExit(f"SMTP {host}:{port} is not reachable: {last_error}")


def send(smtp: smtplib.SMTP, msg: EmailMessage) -> None:
    smtp.send_message(msg)


def build_plain(to_addr: str) -> EmailMessage:
    msg = EmailMessage()
    msg["From"] = "Alice <alice@example.com>"
    msg["To"] = to_addr
    msg["Subject"] = "Welcome to mymail"
    msg["Date"] = formatdate(localtime=True)
    msg["Message-ID"] = make_msgid(domain="example.com")
    msg.set_content(
        "This is a plain-text sample message.\n\n"
        "If you can read this in the app, IMAP fetch is working."
    )
    return msg


def build_japanese(to_addr: str) -> EmailMessage:
    msg = EmailMessage()
    msg["From"] = "山田太郎 <taro@example.com>"
    msg["To"] = to_addr
    msg["Subject"] = "会議の件"
    msg["Date"] = formatdate(localtime=True)
    msg["Message-ID"] = make_msgid(domain="example.com")
    msg.set_content(
        "来週の会議は火曜の10時からです。\n"
        "全文検索テスト用の本文です。キーワード: 会議室 予約。\n"
    )
    return msg


def build_html(to_addr: str) -> EmailMessage:
    msg = EmailMessage()
    msg["From"] = "News <news@example.com>"
    msg["To"] = to_addr
    msg["Subject"] = "HTML newsletter"
    msg["Date"] = formatdate(localtime=True)
    msg["Message-ID"] = make_msgid(domain="example.com")
    msg.set_content("HTML fallback text for clients that do not render HTML.")
    msg.add_alternative(
        """\
<html>
  <body>
    <h1>Hello HTML</h1>
    <p>このメールは HTML です。</p>
    <script>alert('xss')</script>
    <img src="https://evil.example/tracker.png" alt="tracker">
    <p>リモート画像とスクリプトはアプリ側で無効化されます。</p>
  </body>
</html>
""",
        subtype="html",
    )
    return msg


def build_second_japanese(to_addr: str) -> EmailMessage:
    msg = EmailMessage()
    msg["From"] = "佐藤 <sato@example.com>"
    msg["To"] = to_addr
    msg["Subject"] = "添付なしの連絡"
    msg["Date"] = formatdate(localtime=True)
    msg["Message-ID"] = make_msgid(domain="example.com")
    msg.set_content("ランチの場所を変更しました。新しい店は駅前です。")
    return msg


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=3025)
    parser.add_argument("--to", default="testuser@localhost")
    args = parser.parse_args()

    wait_smtp(args.host, args.smtp_port)
    with smtplib.SMTP(args.host, args.smtp_port, timeout=10) as smtp:
        send(smtp, build_plain(args.to))
        send(smtp, build_japanese(args.to))
        send(smtp, build_html(args.to))
        send(smtp, build_second_japanese(args.to))

    print(f"Seeded 4 messages to {args.to} via {args.host}:{args.smtp_port}")
    print("IMAP login: user=testuser  password=testpass  host=127.0.0.1 port=3143 TLS=none")
    return 0


if __name__ == "__main__":
    sys.exit(main())
