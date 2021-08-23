#ifndef ARDUINO_INKPLATE10
#error "Wrong board selection for this example, please select Inkplate 10 in the boards menu."
#endif

#include <WiFi.h>
#include <HTTPClient.h>

#include "Inkplate.h"

// WIFI config
#include "wifi_config.h"

Inkplate display(INKPLATE_3BIT);

size_t http_request(char *url, byte *buffer, size_t buffer_size);
void render(uint8_t *raw_image, size_t nBytes);

void setup()
{
  Serial.begin(115200);

  // Check PSRAM is working
  log_d("Total heap: %d", ESP.getHeapSize());
  log_d("Free heap: %d", ESP.getFreeHeap());
  log_d("Total PSRAM: %d", ESP.getPsramSize());
  log_d("Free PSRAM: %d", ESP.getFreePsram());

  initWiFi();

  display.begin();

  delay(100);

  size_t buffer_size = E_INK_WIDTH * E_INK_HEIGHT / 2 + 1;
  log_d("Allocatig %d bytes in PSRAM for image retrieval.", buffer_size);
  byte *buffer = (byte *)ps_malloc(buffer_size);
  if (buffer == nullptr)
  {
    log_d("Could not allocate memory!");
    delay(5000);
    ESP.restart();
  }

  size_t received = http_request("http://192.168.178.49:8000/comic/inkplate", buffer, buffer_size);

  log_d("Received bytes %d, expected %d", received, buffer_size - 1);

  if (received == buffer_size - 1)
  {
    log_d("Rendering received image...");
    render(buffer, buffer_size - 1);
  }

  free(buffer);
}

void loop()
{
}

size_t http_request(char *url, byte *buffer, size_t buffer_size)
{
  HTTPClient http;
  size_t bytes_read = 0;

  http.begin(url);

  log_d("Http request: GET %s with buffer of size %d", url, buffer_size);
  int httpCode = http.GET();
  if (httpCode > 0)
  {
    if (httpCode == HTTP_CODE_OK)
    {
      int content_length = http.getSize();
      WiFiClient *stream = http.getStreamPtr();
      while (http.connected() && (content_length == -1 || bytes_read < content_length))
      {
        size_t stream_avail = stream->available();
        if (stream_avail)
        {
          int last_read = stream->readBytes(buffer + bytes_read, (buffer_size - bytes_read) < stream_avail ? buffer_size - bytes_read : stream_avail);
          bytes_read += last_read;

          if (bytes_read == buffer_size)
          {
            http.end();
            return bytes_read;
          }
        }
        delay(1);
      }
    }
  }
  else
  {
    log_d("Http GET failed: %s", http.errorToString(httpCode).c_str());
    return 0;
  }

  http.end();

  return bytes_read;
}

void render(uint8_t *raw_image, size_t nBytes)
{
  size_t i;
  uint32_t x, y;
  for (i = 0; i < nBytes; i++)
  {
    y = i / (E_INK_WIDTH / 2);
    x = (i % (E_INK_WIDTH / 2)) * 2;
    display.drawPixel(x, y, (raw_image[i] >> 4) >> 1);
    display.drawPixel(x + 1, y, (raw_image[i] & 0x0f) >> 1);
  }
  display.display();
}
