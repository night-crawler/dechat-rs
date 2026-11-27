#!/bin/bash

# ==============================================================================
# DECHAT-RS MANAGER
# Script to manage multiple dechat-rs configurations as systemd services.
# ==============================================================================

# Output colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check Root
if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}Please run this script as root (sudo).${NC}"
  exit
fi

# Find dechat-rs executable
DECHAT_BIN=$(which dechat-rs)
if [ -z "$DECHAT_BIN" ]; then
    echo -e "${RED}Error: dechat-rs not found in PATH.${NC}"
    echo "Please install dechat-rs first (refer to documentation)."
    exit 1
fi

SYSTEMD_PATH="/etc/systemd/system"

# Function: Show banner
show_header() {
    clear
    echo -e "${YELLOW}=========================================${NC}"
    echo -e "${YELLOW}      DECHAT-RS SERVICE MANAGER          ${NC}"
    echo -e "${YELLOW}=========================================${NC}"
}

# Function: Create new configuration
create_config() {
    show_header
    echo -e "Detecting devices...\n"
    $DECHAT_BIN list -n
    echo -e "-----------------------------------------"
    
    # 1. Choose Device
    echo -e "\n${GREEN}[STEP 1] Device Identification${NC}"
    echo "Enter a unique string to identify the device (e.g., 'Logitech PRO')."
    echo "The filter uses 'starts with' mode (s:Name)."
    read -p "Identifier string: " DEVICE_NAME
    
    if [ -z "$DEVICE_NAME" ]; then
        echo -e "${RED}Invalid name.${NC}"
        read -p "Press Enter to return to menu..."
        return
    fi

    # Sanitize name for filename (spaces/special chars to hyphens)
    SAFE_NAME=$(echo "$DEVICE_NAME" | sed -e 's/[^A-Za-z0-9._-]/ /g' | xargs | tr ' ' '-' | tr '[:upper:]' '[:lower:]')
    SERVICE_FILE="dechat-${SAFE_NAME}.service"

    # 2. Debounce Time
    echo -e "\n${GREEN}[STEP 2] Debounce Configuration${NC}"
    read -p "Enter debounce time in ms (Default: 45): " DEBOUNCE_TIME
    DEBOUNCE_TIME=${DEBOUNCE_TIME:-45}

    # 3. Restart Policy
    echo -e "\n${GREEN}[STEP 3] Restart Policy${NC}"
    echo "1) always (Recommended - Always restart)"
    echo "2) on-failure (Restart only on crash)"
    echo "3) no (No automatic restart)"
    read -p "Choice (Default: 1): " RESTART_OPT
    
    case $RESTART_OPT in
        2) RESTART_POLICY="on-failure" ;;
        3) RESTART_POLICY="no" ;;
        *) RESTART_POLICY="always" ;;
    esac

    # Create Systemd File
    echo -e "\n${GREEN}[INFO] Generating service file: $SERVICE_FILE${NC}"
    
    cat > "$SYSTEMD_PATH/$SERVICE_FILE" <<EOF
[Unit]
Description=Dechat-rs Service for $DEVICE_NAME
After=network.target inputs.service

[Service]
Type=simple
ExecStart=$DECHAT_BIN de-chatter -t 0:1000:$DEBOUNCE_TIME -n s:'$DEVICE_NAME'
Restart=$RESTART_POLICY
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

    echo -e "File created at $SYSTEMD_PATH/$SERVICE_FILE"
    
    # Activation
    echo -e "Reloading daemons and starting service..."
    systemctl daemon-reload
    systemctl enable "$SERVICE_FILE"
    systemctl start "$SERVICE_FILE"
    
    if systemctl is-active --quiet "$SERVICE_FILE"; then
        echo -e "${GREEN}SUCCESS! Service is active and running.${NC}"
    else
        echo -e "${RED}WARNING: Service created but does not seem active. Check 'systemctl status $SERVICE_FILE'${NC}"
    fi
    
    read -p "Press Enter to continue..."
}

# Function: Manage existing services
manage_services() {
    while true; do
        show_header
        echo -e "${GREEN}Active Configurations:${NC}\n"
        
        # Array to store found services
        SERVICES=($(ls $SYSTEMD_PATH/dechat-*.service 2>/dev/null | xargs -n 1 basename))
        
        if [ ${#SERVICES[@]} -eq 0 ]; then
            echo "No dechat services configured."
            echo "-----------------------------------------"
            read -p "Press Enter to return to menu..."
            return
        fi

        # List services with status
        i=1
        for svc in "${SERVICES[@]}"; do
            STATUS=$(systemctl is-active "$svc")
            COLOR=$RED
            if [ "$STATUS" == "active" ]; then COLOR=$GREEN; fi
            echo -e "$i) $svc - [${COLOR}$STATUS${NC}]"
            ((i++))
        done

        echo -e "\nSelect a number to manage the service (or 'q' to go back):"
        read -p "> " CHOICE

        if [[ "$CHOICE" == "q" ]]; then return; fi

        # Validate input
        if ! [[ "$CHOICE" =~ ^[0-9]+$ ]] || [ "$CHOICE" -lt 1 ] || [ "$CHOICE" -gt ${#SERVICES[@]} ]; then
            continue
        fi

        SELECTED_SVC="${SERVICES[$((CHOICE-1))]}"
        
        echo -e "\nManaging: ${YELLOW}$SELECTED_SVC${NC}"
        echo "1) Start"
        echo "2) Stop"
        echo "3) Restart"
        echo "4) Disable & Delete (Remove configuration)"
        echo "5) Back"
        read -p "> " ACTION

        case $ACTION in
            1) systemctl start "$SELECTED_SVC" ;;
            2) systemctl stop "$SELECTED_SVC" ;;
            3) systemctl restart "$SELECTED_SVC" ;;
            4) 
                echo -e "${RED}Are you sure you want to delete $SELECTED_SVC? (y/n)${NC}"
                read -p "> " CONFIRM
                if [[ "$CONFIRM" == "y" ]]; then
                    systemctl stop "$SELECTED_SVC"
                    systemctl disable "$SELECTED_SVC"
                    rm "$SYSTEMD_PATH/$SELECTED_SVC"
                    systemctl daemon-reload
                    echo "Service removed."
                    sleep 1
                fi
                ;;
            *) ;;
        esac
    done
}

# Main Loop
while true; do
    show_header
    echo "1) Create new device configuration"
    echo "2) Manage/Remove existing configurations"
    echo "3) Exit"
    echo ""
    read -p "Select option: " OPTION

    case $OPTION in
        1) create_config ;;
        2) manage_services ;;
        3) exit 0 ;;
        *) echo "Invalid option." ;;
    esac
done
