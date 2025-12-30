import { AccessToken } from 'livekit-server-sdk';

const createToken = async () => {
  const roomName = 'test-room';
  const participantName = 'rust-client-' + Math.floor(Math.random() * 1000);
  
  const apiKey = 'change_me';
  const apiSecret = 'change_me';

  const at = new AccessToken(apiKey, apiSecret, {
      identity: participantName,
      ttl: 24 * 60 * 60, // 24 saat geçerli
    },
  );

  at.addGrant({ roomJoin: true, room: roomName, canPublish: true, canSubscribe: true });

  const token = await at.toJwt();
  console.log('--- TOKEN START ---');
  console.log(token);
  console.log('--- TOKEN END ---');
};

createToken();
