// Configure based on your setup
// After that comment out the following line:
#error Configure your WiFi credentials in 'wifi_config.h'

// Enable to create own access point otherwise the defined one will be joined.
// #define SOFTAP_MODE

#include "util.h"

#ifndef SOFTAP_MODE
const char *ssid = "";
const char *password = "";

ALWAYS_INLINE void initWiFi()
{
  WiFi.mode(WIFI_STA);
  WiFi.begin(ssid, password);
  log_d("Connecting to WiFi with SSID %s and password %s", ssid, password);
  while (WiFi.status() != WL_CONNECTED)
  {
    log_d("Waiting for connection...");
    delay(250);
  }
  log_d("Connected to wifi with ip address %s", WiFi.localIP().toString().c_str());
}

#endif

#ifdef SOFTAP_MODE
const char *ssid = "ESP32-Access-Point";

ALWAYS_INLINE void initWiFi()
{
  log_d("Setting up WiFi AP with SSID %s", ssid);
  WiFi.softAP(ssid);
  IPAddress IP = WiFi.softAPIP();
  log_d("Established access point with SSID %s and ip address %s", ssid, IP.toString().c_str());
}
#endif
