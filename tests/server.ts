const port = Number(Deno.args[0] || 8000);

const socket = Deno.listenDatagram({
  port,
  transport: "udp",
  hostname: "0.0.0.0",
});

console.log(`UDP 聊天服务端已启动，监听端口 ${port}`);
console.log("输入消息后按回车发送 (Ctrl+C 退出)");

let lastClient: Deno.NetAddr | null = null;

const handleInput = async () => {
  for await (const line of Deno.stdin.readable) {
    const text = new TextDecoder().decode(line).trim();
    if (text && lastClient) {
      await socket.send(
        new TextEncoder().encode(text),
        lastClient,
      );
    }
  }
};

handleInput();

while (true) {
  try {
    const [data, remote] = await socket.receive();
    lastClient = remote as Deno.NetAddr;
    const message = new TextDecoder().decode(data);
    console.log(`[客户端 ${lastClient.hostname}:${lastClient.port}]`, message);
  } catch (err) {
    console.error("[接收错误]", err);
    break;
  }
}
