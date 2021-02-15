package main

import (
	"crypto/tls"
	"encoding/json"
	"io/ioutil"
	"net"
	"sync"

	nats "github.com/nats-io/nats.go"
	log "github.com/sirupsen/logrus"
)

const (
	// DefaultMaxDataGramSize is the default maximum size of a UDP packet
	DefaultMaxDataGramSize = 1024
)

// Config defines the config to be given for magicportal in a json file.
type Config struct {
	MulticastGroups []MulticastGroup `json:"multicast_groups"`
	Mode            string           `json:"mode"`
	NatsURL         string           `json:"nats_url"`
	TLS             bool             `json:"tls_enabled"`

	// If this is enabled data will be sent as unicast udp data instead of multicast
	SendAsUnicast bool `json:"send_as_unicast"`

	// Map of multicast group and the corresponding unicast address
	UnicastAddrs map[string]string `json:"unicast_addrs"`

	MaxPacketSize int `json:"max_packet_size"`
}

// MulticastGroup contains address and interface
type MulticastGroup struct {
	MulticastAddr string `json:"multicast_addr"`
	Interface     string `json:"interface"`
}

func main() {

	config, err := readConfig()

	if err != nil {
		log.Fatal(err)
	}

	var nc *nats.Conn

	if config.TLS {
		conf := &tls.Config{
			InsecureSkipVerify: true,
		}
		// Connect to Nats
		nc, err = nats.Connect(config.NatsURL, nats.Secure(conf))
	} else {
		// Connect to Nats
		nc, err = nats.Connect(config.NatsURL)
	}

	if err != nil {
		log.Fatal("Cannot connect to NATS. Error: ", err)
	}

	log.Println("Connected to NATS.")

	addressesCount := len(config.MulticastGroups)

	var wg sync.WaitGroup

	wg.Add(addressesCount)

	// If mode is forwarder start server goroutines
	if config.Mode == "forwarder" {
		for _, grp := range config.MulticastGroups {
			go serveMulticastUDP(grp.MulticastAddr, grp.Interface, &wg, nc, config)
		}
	} else if config.Mode == "agent" {
		for _, grp := range config.MulticastGroups {
			var connAddr *net.UDPAddr
			var err error
			if config.SendAsUnicast {
				udpAddr, ok := config.UnicastAddrs[grp.MulticastAddr]
				if !ok {
					continue
				}

				connAddr, err = net.ResolveUDPAddr("udp", udpAddr)
			} else {
				connAddr, err = net.ResolveUDPAddr("udp", grp.MulticastAddr)
			}

			if err != nil {
				log.Fatalf("Cannot resolve %v", grp.MulticastAddr)
			}

			conn, err := net.DialUDP("udp", nil, connAddr)

			nc.Subscribe(grp.MulticastAddr, func(msg *nats.Msg) {
				conn.Write(msg.Data)
			})
		}
	}

	wg.Wait()
}

func readConfig() (Config, error) {
	data, err := ioutil.ReadFile("config.json")

	if err != nil {
		return Config{}, err
	}

	config := Config{}
	err = json.Unmarshal(data, &config)
	if err != nil {
		return Config{}, err
	}
	return config, nil
}

func serveMulticastUDP(multicastAddr string, inf string, wg *sync.WaitGroup, nc *nats.Conn, cfg Config) {
	defer wg.Done()
	addr, err := net.ResolveUDPAddr("udp", multicastAddr)

	if err != nil {
		log.Fatalf("Got error while resolving UDP address: %v", err)
	}

	interf := net.Interface{
		Name: inf,
	}

	l, err := net.ListenMulticastUDP("udp", &interf, addr)

	if err != nil {
		log.Fatalf("Got error while listening to multicast address: %v", err)
	}

	maxDataGramSize := DefaultMaxDataGramSize

	if cfg.MaxPacketSize != 0 {
		maxDataGramSize = cfg.MaxPacketSize
	}

	l.SetReadBuffer(maxDataGramSize)

	log.Printf("Listening for %v", multicastAddr)

	b := make([]byte, 0, maxDataGramSize)
	for {
		len, _, err := l.ReadFromUDP(b)
		if err != nil {
			log.Fatal("ReadFromUDP failed:", err)
		}

		log.Printf("Number of bytes read: %d", len)

		// Publish to Nats
		nc.Publish(multicastAddr, b[:len])

		// Reset b
		b = b[:]
	}
}
