const [targetHost, targetPort, localPort] = Deno.args;

if (!targetHost || !targetPort) {
  console.log(
    "用法: deno run --allow-net --unstable-net client.ts <目标IP> <目标端口> [本地端口]",
  );
  Deno.exit(1);
}

const socket = Deno.listenDatagram({
  port: localPort ? Number(localPort) : 0, // 0 表示随机端口
  transport: "udp",
  hostname: "0.0.0.0",
});

const remoteAddr = {
  hostname: targetHost,
  port: Number(targetPort),
};

console.log(`已连接到服务端 ${targetHost}:${targetPort}`);
console.log("输入消息后按回车发送 (Ctrl+C 退出)");

const handleInput = async () => {
  for await (const line of Deno.stdin.readable) {
    const text = new TextDecoder().decode(line).trim();
    if (text) {
      await socket.send(
        new TextEncoder().encode(text),
        {
          ...remoteAddr,
          transport: "udp",
        },
      );
    }
  }
};

handleInput();

while (true) {
  try {
    const [data, remote] = await socket.receive();
    const message = new TextDecoder().decode(data);
    console.log(
      `[服务器 ${(remote as Deno.NetAddr).hostname}:${
        (remote as Deno.NetAddr).port
      }]`,
      message,
    );
  } catch (err) {
    console.error("[接收错误]", err);
    break;
  }
}
