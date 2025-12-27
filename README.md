# DCTS Native Audio Core (Rust PoC)

Bu proje, LiveKit SFU'sunu kullanarak, tarayıcı (browser) katmanı olmadan doğrudan işletim sistemi ses API'leri (ALSA/PulseAudio) üzerinden yüksek performanslı ses iletimi yapan bir "Proof of Concept" (Kavram Kanıtı) çalışmasıdır.

## 🛠️ Ön Gereksinimler (Linux)

Rust ve ses kütüphanelerini derleyebilmek için sistem paketlerini kurmalısınız:

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install -y pkg-config libssl-dev libasound2-dev libpulse-dev build-essential
```

## 🚀 Çalıştırma

1.  **LiveKit URL ve Token Alın:**
    LiveKit Cloud veya kendi sunucunuzdan bir proje URL'i ve "Join" izni olan bir token oluşturun.

2.  **Çevre Değişkenlerini Ayarlayın:**
    `.env` dosyasını açın ve bilgilerinizi girin:
    ```ini
    LIVEKIT_URL=wss://your-project.livekit.cloud
    LIVEKIT_TOKEN=eyJ...
    ```

3.  **Başlatın:**
    ```bash
    cargo run
    ```

## 🧪 Nasıl Test Edilir?

1.  Uygulamayı çalıştırın (`cargo run`).
2.  Mikrofonunuzun açıldığını terminal çıktısından doğrulayın.
3.  Başka bir cihazdan (tarayıcıdan veya telefondan) aynı odaya bağlanın.
4.  Konuştuğunuzda sesinizin diğer tarafa **ne kadar hızlı** gittiğine (gecikme) dikkat edin.
5.  Diğer taraftan konuşup bu terminal uygulamasından sesi duyun.

## ⚠️ Notlar ve Sınırlamalar

*   **Audio Mixing:** Bu demo, birden fazla kişi aynı anda konuştuğunda sesleri basitçe arka arkaya ekler (mixer yoktur). Gürültü olabilir.
*   **Resampling:** Mikrofonunuz 44.1kHz ve LiveKit 48kHz ise sesiniz biraz hızlı/yavaş (sincap gibi) gidebilir. Gerçek uygulamada `rubato` gibi bir kütüphane ile "Resampling" eklenmelidir.
*   **Echo Cancellation:** Şu an saf ham ses kullanıyoruz. Yankı engelleme (AEC) yoktur. Hoparlör sesini mikrofon tekrar alabilir. Kulaklık kullanmanız önerilir.
