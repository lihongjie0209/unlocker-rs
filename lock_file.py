import time
import sys

# 以写模式打开文件并保持打开状态
with open("windows_test_file.txt", "a") as f:
    print("文件已打开，按 Ctrl+C 退出...")
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("退出...")