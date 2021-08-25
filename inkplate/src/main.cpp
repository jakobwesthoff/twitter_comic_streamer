#ifndef ARDUINO_INKPLATE10
#error "Wrong board selection, please select Inkplate 10 in the boards menu."
#endif

#include <WiFi.h>
#include <HTTPClient.h>

#include "Inkplate.h"
#include "driver/rtc_io.h"

#include "battery.h";

// WIFI config
#include "wifi_config.h"

// Conversion factor for micro seconds to seconds
#define uS_TO_S_FACTOR 1000000
// Time ESP32 will go to sleep (in seconds)
#define TIME_TO_SLEEP 300

Inkplate display(INKPLATE_3BIT);

size_t http_request(char *url, byte *buffer, size_t buffer_size);
void render(uint8_t *raw_image, size_t nBytes);
void setup_mcp();
void goto_sleep(uint64_t);
void log_wakeup_reason();

void setup()
{
  Serial.begin(115200);

  // setup_mcp();

  log_wakeup_reason();

  // Check PSRAM is working
  log_d("Total heap: %d", ESP.getHeapSize());
  log_d("Free heap: %d", ESP.getFreeHeap());
  log_d("Total PSRAM: %d", ESP.getPsramSize());
  log_d("Free PSRAM: %d", ESP.getFreePsram());

  initWiFi();

  display.begin();
  display.setTextSize(3);
  display.setTextColor(0, 7);
  display.setTextWrap(true);


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

  checkBattery(&display);
  display.display();

  goto_sleep(TIME_TO_SLEEP * uS_TO_S_FACTOR);
}

void loop()
{
  // Never reached because of sleep
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
}

void setup_mcp()
{
  byte touchPadPin = 10;
  display.pinModeInternal(MCP23017_INT_ADDR, display.mcpRegsInt, touchPadPin, INPUT);
  display.setIntOutputInternal(MCP23017_INT_ADDR, display.mcpRegsInt, 1, false, false, HIGH);
  display.setIntPinInternal(MCP23017_INT_ADDR, display.mcpRegsInt, touchPadPin, RISING);
}

void goto_sleep(uint64_t micro_seconds)
{
  log_d("Preparing to sleep");
  // timer deepsleep
  esp_sleep_enable_timer_wakeup(micro_seconds);

  // touchpad pad interrupt pin
  // esp_sleep_enable_ext0_wakeup(GPIO_NUM_34, 1);
  esp_sleep_enable_ext0_wakeup(GPIO_NUM_36, 0);

  rtc_gpio_isolate(GPIO_NUM_12);

  // goto sleep for real.
  log_d("Going to sleep...");
  delay(250);
  esp_deep_sleep_start();
}

void log_wakeup_reason()
{
  esp_sleep_wakeup_cause_t wakeup_reason;
  wakeup_reason = esp_sleep_get_wakeup_cause();
  switch (wakeup_reason)
  {
  case ESP_SLEEP_WAKEUP_EXT0:
    log_d("Wakeup caused by external signal using RTC_IO");
    break;
  case ESP_SLEEP_WAKEUP_EXT1:
    log_d("Wakeup caused by external signal using RTC_CNTL");
    break;
  case ESP_SLEEP_WAKEUP_TIMER:
    log_d("Wakeup caused by timer");
    break;
  case ESP_SLEEP_WAKEUP_TOUCHPAD:
    log_d("Wakeup caused by touchpad");
    break;
  case ESP_SLEEP_WAKEUP_ULP:
    log_d("Wakeup caused by ULP program");
    break;
  default:
    log_d("Wakeup was not caused by deep sleep");
    break;
  }
}
