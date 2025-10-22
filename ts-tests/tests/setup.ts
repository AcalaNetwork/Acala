process.on('unhandledRejection', (reason: any) => {
  const message = reason?.message || String(reason);
  
  if (
    message.includes('Normal Closure') ||
    message.includes('disconnected from ws://') ||
    message.includes('1000::')
  ) {
    return;
  }
  
  console.error('Unhandled Rejection:', reason);
  throw reason;
});

process.on('uncaughtException', (error: Error) => {
  if (
    error.message.includes('Normal Closure') ||
    error.message.includes('disconnected from ws://')
  ) {
    return;
  }
  console.error('Uncaught Exception:', error);
  throw error;
});