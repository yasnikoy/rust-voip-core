import { AccessToken } from 'livekit-server-sdk';

const createToken = async () => {
  const roomName = 'test-room';
  const participantName = 'viewer-1'; // Different identity
  
  const apiKey = 'change_me';
  const apiSecret = 'change_me';

  const at = new AccessToken(apiKey, apiSecret, {
      identity: participantName,
      ttl: 24 * 60 * 60,
    },
  );

  at.addGrant({ roomJoin: true, room: roomName, canPublish: false, canSubscribe: true }); // Viewer only

  const token = await at.toJwt();
  console.log('--- VIEWER TOKEN START ---');
  console.log(token);
  console.log('--- VIEWER TOKEN END ---');
};

createToken();
