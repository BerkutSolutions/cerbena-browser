package main

import (
	"encoding/binary"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"os"
	"strconv"
	"time"
)

func main() {
	listenAddr := flag.String("listen", "0.0.0.0:17890", "SOCKS5 listen address")
	flag.Parse()

	logger := log.New(os.Stdout, "[container-socks] ", log.LstdFlags|log.LUTC)
	listener, err := net.Listen("tcp", *listenAddr)
	if err != nil {
		logger.Fatalf("listen failed: %v", err)
	}
	logger.Printf("ready on %s", *listenAddr)

	for {
		conn, err := listener.Accept()
		if err != nil {
			logger.Printf("accept failed: %v", err)
			time.Sleep(100 * time.Millisecond)
			continue
		}
		go handleConn(logger, conn)
	}
}

func handleConn(logger *log.Logger, conn net.Conn) {
	defer conn.Close()
	_ = conn.SetDeadline(time.Now().Add(30 * time.Second))

	header := make([]byte, 2)
	if _, err := io.ReadFull(conn, header); err != nil {
		logger.Printf("read greeting failed: %v", err)
		return
	}
	if header[0] != 0x05 {
		logger.Printf("unsupported version: %d", header[0])
		return
	}
	methods := make([]byte, int(header[1]))
	if _, err := io.ReadFull(conn, methods); err != nil {
		logger.Printf("read methods failed: %v", err)
		return
	}
	if _, err := conn.Write([]byte{0x05, 0x00}); err != nil {
		logger.Printf("write greeting response failed: %v", err)
		return
	}

	requestHeader := make([]byte, 4)
	if _, err := io.ReadFull(conn, requestHeader); err != nil {
		logger.Printf("read request header failed: %v", err)
		return
	}
	if requestHeader[0] != 0x05 || requestHeader[1] != 0x01 {
		writeSocksFailure(conn, 0x07)
		return
	}

	targetHost, targetPort, err := readTarget(conn, requestHeader[3])
	if err != nil {
		logger.Printf("read target failed: %v", err)
		writeSocksFailure(conn, 0x08)
		return
	}

	upstream, err := net.DialTimeout("tcp", net.JoinHostPort(targetHost, strconv.Itoa(int(targetPort))), 15*time.Second)
	if err != nil {
		logger.Printf("dial %s:%d failed: %v", targetHost, targetPort, err)
		writeSocksFailure(conn, 0x05)
		return
	}
	defer upstream.Close()

	if _, err := conn.Write([]byte{0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0}); err != nil {
		logger.Printf("write connect success failed: %v", err)
		return
	}

	_ = conn.SetDeadline(time.Time{})
	_ = upstream.SetDeadline(time.Time{})
	bridge(logger, conn, upstream)
}

func readTarget(conn net.Conn, atyp byte) (string, uint16, error) {
	switch atyp {
	case 0x01:
		addr := make([]byte, 4)
		if _, err := io.ReadFull(conn, addr); err != nil {
			return "", 0, err
		}
		port, err := readPort(conn)
		if err != nil {
			return "", 0, err
		}
		return net.IP(addr).String(), port, nil
	case 0x03:
		size := make([]byte, 1)
		if _, err := io.ReadFull(conn, size); err != nil {
			return "", 0, err
		}
		host := make([]byte, int(size[0]))
		if _, err := io.ReadFull(conn, host); err != nil {
			return "", 0, err
		}
		port, err := readPort(conn)
		if err != nil {
			return "", 0, err
		}
		return string(host), port, nil
	case 0x04:
		addr := make([]byte, 16)
		if _, err := io.ReadFull(conn, addr); err != nil {
			return "", 0, err
		}
		port, err := readPort(conn)
		if err != nil {
			return "", 0, err
		}
		return net.IP(addr).String(), port, nil
	default:
		return "", 0, fmt.Errorf("unsupported atyp %d", atyp)
	}
}

func readPort(conn net.Conn) (uint16, error) {
	portBytes := make([]byte, 2)
	if _, err := io.ReadFull(conn, portBytes); err != nil {
		return 0, err
	}
	return binary.BigEndian.Uint16(portBytes), nil
}

func writeSocksFailure(conn net.Conn, code byte) {
	_, _ = conn.Write([]byte{0x05, code, 0x00, 0x01, 0, 0, 0, 0, 0, 0})
}

func bridge(logger *log.Logger, client net.Conn, upstream net.Conn) {
	done := make(chan struct{}, 2)
	copyHalf := func(dst net.Conn, src net.Conn) {
		_, err := io.Copy(dst, src)
		if tcp, ok := dst.(*net.TCPConn); ok {
			_ = tcp.CloseWrite()
		}
		if err != nil && !isUseOfClosed(err) {
			logger.Printf("bridge copy failed: %v", err)
		}
		done <- struct{}{}
	}

	go copyHalf(upstream, client)
	go copyHalf(client, upstream)
	<-done
	<-done
}

func isUseOfClosed(err error) bool {
	if err == nil {
		return false
	}
	return err == io.EOF
}
